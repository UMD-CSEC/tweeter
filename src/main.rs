use std::{
    net::SocketAddr,
    ops::Deref,
    sync::{Arc, Mutex}, collections::HashMap,
};

use anyhow::anyhow;
use axum::{
    extract::{FromRef, State, Path},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response, Result},
    routing::{get, post},
    Form, Router,
};

use axum_extra::extract::{
    cookie::{Cookie, Key},
    SignedCookieJar,
};
use chrono::{TimeZone, Local};
use minijinja::{context, path_loader, Environment};
use serde::Deserialize;

use tower_http::{trace::TraceLayer, services::ServeDir};

use db::{AppDb, MemDb, User, UserRole};
use tracing::Level;
use tracing_subscriber::{filter, prelude::*};
use urlencoding::encode;

use crate::db::Post;

mod db;

const VIEWS_DIR: &str = "views/";

pub struct AppState<D: AppDb>(Arc<InnerState<D>>);

impl<D: AppDb> Clone for AppState<D> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<D: AppDb> AppState<D> {
    pub fn new(db: D, env: Environment<'static>) -> Self {
        Self(Arc::new(InnerState {
            db: Mutex::new(db),
            key: Key::generate(),
            env,
        }))
    }
}

impl<D: AppDb> Deref for AppState<D> {
    type Target = InnerState<D>;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

impl<D: AppDb> FromRef<AppState<D>> for Key {
    fn from_ref(state: &AppState<D>) -> Self {
        state.0.key.clone()
    }
}

pub struct InnerState<D: AppDb> {
    pub db: Mutex<D>,
    pub key: Key,
    pub env: Environment<'static>,
}

#[tokio::main]
async fn main() {
    let filter = filter::Targets::new()
        .with_target("tower_http::trace::on_request", Level::DEBUG)
        .with_target("tower_http::trace::on_response", Level::DEBUG)
        .with_target("tower_http::trace::make_span", Level::DEBUG)
        .with_default(Level::INFO);

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

    let mut env = Environment::new();
    env.set_loader(path_loader(VIEWS_DIR));
    env.add_filter("format_time", |timestamp: u64| {
        let datetime = Local.timestamp_opt(timestamp as i64, 0).unwrap();
        datetime.format("%b %-d, %Y %-I:%M:%S").to_string()
    });

    let state = AppState::new(MemDb::new(), env);
    {
        let mut db = state.db.lock().unwrap();
        let admin = User::new("admin", "pepegaman123", UserRole::Admin, true);
        db.add_user(admin).unwrap();
    }

    let admin_router = Router::new()
        .route("/admin", get(get_admin))
        .route("/admin/users", get(get_users_admin).post(post_users_admin))
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state.clone(), auth_admin));

    let app = Router::new()
        .nest_service("/assets", ServeDir::new("assets"))
        .route("/", get(get_index))
        .route("/register", get(get_register).post(post_register))
        .route("/login", get(get_login).post(post_login))
        .route("/logout", post(post_logout))
        .route("/create_post", get(get_create_post).post(post_create_post))
        .route("/profile/:user_id", get(get_profile))
        .route("/settings", get(get_settings).post(post_settings))
        .merge(admin_router)
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    axum::Server::bind(&"127.0.0.1:1447".parse().unwrap())
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}

async fn get_index(
    State(state): State<AppState<impl AppDb>>,
    jar: SignedCookieJar,
) -> Result<Html<String>> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let posts = db
        .get_posts()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // yes this is slow
    // but prolly ok for small amount of users B)
    let user_map: HashMap<u64, User> = db.get_users().unwrap().into_iter().map(|user| {
        (user.id(), user)
    }).collect();

    // get current user
    let user = if let Some(username) = jar.get("user") {
        db.get_user_by_name(username.value()).ok()
    } else {
        None
    };

    let tmpl = state.env.get_template("index.html").unwrap();
    Ok(Html(
        tmpl.render(context! {
            user,
            user_map,
            posts
        })
        .unwrap(),
    ))
}

async fn get_register(State(state): State<AppState<impl AppDb>>, jar: SignedCookieJar) -> Response {
    if jar.get("user").is_some() {
        Redirect::to("/").into_response()
    } else {
        let tmpl = state.env.get_template("register.html").unwrap();
        Html(tmpl.render(context! {}).unwrap()).into_response()
    }
}

#[derive(Deserialize)]
struct SignUp {
    username: String,
    password: String,
}

async fn post_register(
    State(state): State<AppState<impl AppDb>>,
    mut jar: SignedCookieJar,
    Form(sign_up): Form<SignUp>,
) -> Result<(SignedCookieJar, Redirect)> {
    if jar.get("user").is_none() {
        let new_user = User::new(&sign_up.username, &sign_up.password, UserRole::User, false);

        state
            .db
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .add_user(new_user)
            .map_err(|err| {
                let err_str = format!("failed to add user: {}", err);
                Redirect::to(&format!("/register#{}", encode(&err_str)))
            })?;

        jar = jar.add(Cookie::new("user", sign_up.username));
    }

    Ok((jar, Redirect::to("/")))
}

async fn get_login(State(state): State<AppState<impl AppDb>>, jar: SignedCookieJar) -> Response {
    if jar.get("user").is_some() {
        Redirect::to("/").into_response()
    } else {
        let tmpl = state.env.get_template("login.html").unwrap();
        Html(tmpl.render(context! {}).unwrap()).into_response()
    }
}

#[derive(Deserialize)]
struct SignIn {
    username: String,
    password: String,
}

async fn post_login(
    State(state): State<AppState<impl AppDb>>,
    mut jar: SignedCookieJar,
    Form(sign_in): Form<SignIn>,
) -> Result<(SignedCookieJar, Redirect)> {
    if jar.get("user").is_none() {
        let db = state
            .db
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        db.get_user_by_name(&sign_in.username)
            .and_then(|user| {
                if user.check_password(&sign_in.password) {
                    Ok(user)
                } else {
                    Err(anyhow!("incorrect password"))
                }
            })
            .map_err(|_| {
                Redirect::to(&format!(
                    "/login?err={}",
                    encode("incorrect username/password")
                ))
            })?;

        jar = jar.add(Cookie::new("user", sign_in.username));
    }

    Ok((jar, Redirect::to("/")))
}

async fn post_logout(
    jar: SignedCookieJar,
) -> Result<(SignedCookieJar, Redirect)> {
    let jar = if let Some(username) = jar.get("user") {
        jar.remove(username)
    } else {
        jar
    };
    Ok((jar, Redirect::to("/")))
}

async fn get_create_post(
    State(state): State<AppState<impl AppDb>>,
    jar: SignedCookieJar
) -> Response {
    if jar.get("user").is_none() {
        return Redirect::to("/login").into_response();
    }

    let tmpl = state.env.get_template("create_post.html").unwrap();
    Html(tmpl.render(context! {}).unwrap()).into_response()
}

#[derive(Deserialize)]
struct CreatePost {
    contents: String,
}

async fn post_create_post(
    State(state): State<AppState<impl AppDb>>,
    jar: SignedCookieJar,
    Form(form): Form<CreatePost>,
) -> Result<Redirect> {
    let Some(username) = jar.get("user") else {
        return Ok(Redirect::to("/login"));
    };

    let mut db = state.db.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let user = db.get_user_by_name(username.value()).map_err(|_| Redirect::to("/logout"))?;

    let post = Post::new(&user, &form.contents);
    db.add_post(post).unwrap();

    Ok(Redirect::to("/"))
}

async fn get_profile(
    State(state): State<AppState<impl AppDb>>,
    Path(user_id): Path<u64>,
) -> Result<Html<String>> {
    let db = state.db.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let user = db.get_user_by_id(user_id).map_err(|_| (StatusCode::NOT_FOUND, format!("no user with id {}", user_id)))?;

    let tmpl = state.env.get_template("profile.html").unwrap();
    Ok(Html(tmpl.render(context! {
        user
    }).unwrap()))
}

async fn get_settings(
    State(state): State<AppState<impl AppDb>>,
    jar: SignedCookieJar,
) -> Result<Response> {
    if let Some(username) = jar.get("user") {
        let db = state
            .db
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let user = db
            .get_user_by_name(username.value())
            .map_err(|_| Redirect::to("/logout"))?;

        let tmpl = state.env.get_template("settings.html").unwrap();
        Ok(Html(
            tmpl.render(context! {
                user
            })
            .unwrap(),
        )
        .into_response())
    } else {
        Ok(Redirect::to("/login").into_response())
    }
}

#[derive(Deserialize)]
struct Settings {
    currpass: String,
    newpass: String,
    bio: String,
}

async fn post_settings(
    State(state): State<AppState<impl AppDb>>,
    jar: SignedCookieJar,
    Form(settings): Form<Settings>,
) -> Result<Response> {
    if let Some(username) = jar.get("user") {
        let mut db = state
            .db
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let mut user = db
            .get_user_by_name(username.value())
            .map_err(|_| Redirect::to("/logout"))?;

        if !settings.newpass.is_empty() {
            user.change_password(&settings.currpass, &settings.newpass)
                .map_err(|e| {
                    let err_str = e.to_string();
                    let msg = encode(&err_str);
                    Redirect::to(&format!("/settings?err={}", msg))
                })?;
        }
        user.set_bio(&settings.bio);

        db.update_user(user)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(Redirect::to(&format!(
            "/settings?success={}",
            "Successfully updated settings"
        ))
        .into_response())
    } else {
        Ok(Redirect::to("/login").into_response())
    }
}

async fn auth_admin<B>(
    State(state): State<AppState<impl AppDb>>,
    jar: SignedCookieJar,
    request: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    if let Some(username) = jar.get("user") {
        let user = {
            let db = state
                .db
                .lock()
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            db.get_user_by_name(username.value())
                .map_err(|_| StatusCode::UNAUTHORIZED)?
        };

        let resp = if user.role() == UserRole::Admin {
            Ok(next.run(request).await)
        } else {
            Err(StatusCode::UNAUTHORIZED)
        };

        return resp;
    }

    Err(StatusCode::UNAUTHORIZED)
}

async fn get_admin(State(state): State<AppState<impl AppDb>>) -> Html<String> {
    let tmpl = state.env.get_template("admin/index.html").unwrap();
    Html(tmpl.render(context! {}).unwrap())
}

async fn get_users_admin(State(state): State<AppState<impl AppDb>>) -> Result<Html<String>> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let users = db
        .get_users()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let tmpl = state.env.get_template("admin/users.html").unwrap();
    Ok(Html(tmpl.render(context! { users }).unwrap()))
}

#[derive(Deserialize)]
enum UserCmd {
    GrantBlue,
    RemoveBlue,
}

#[derive(Deserialize)]
struct UpdateUser {
    id: u64,
    cmd: UserCmd,
}

async fn post_users_admin(
    State(state): State<AppState<impl AppDb>>,
    Form(form): Form<UpdateUser>,
) -> Result<Redirect> {
    let mut db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut user = db.get_user_by_id(form.id).map_err(|_| StatusCode::BAD_REQUEST)?;
    match form.cmd {
        UserCmd::GrantBlue => user.set_blue(true),
        UserCmd::RemoveBlue => user.set_blue(false),
    };

    db.update_user(user).unwrap();

    Ok(Redirect::to("/admin/users"))
}