use anyhow::{anyhow, bail, Result};
use chrono::Local;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct User {
    id: u64,
    name: String,
    password: String,
    role: UserRole,

    blue: bool,

    bio: String,
}

#[derive(Debug, Copy, Clone, Serialize, PartialEq)]
pub enum UserRole {
    User,
    Admin,
}

impl User {
    pub fn new(name: &str, password: &str, role: UserRole, blue: bool) -> Self {
        Self {
            id: 0,
            name: name.to_owned(),
            password: password.to_owned(),
            role,
            blue,
            bio: String::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn role(&self) -> UserRole {
        self.role
    }

    pub fn blue(&self) -> bool {
        self.blue
    }

    pub fn set_bio(&mut self, bio: &str) {
        self.bio = bio.to_owned();
    }

    pub fn set_role(&mut self, role: UserRole) {
        self.role = role;
    }

    pub fn set_blue(&mut self, blue: bool) {
        self.blue = blue;
    }

    pub fn check_password(&self, password: &str) -> bool {
        self.password == password
    }

    pub fn change_password(&mut self, curr_pass: &str, new_pass: &str) -> Result<()> {
        if self.check_password(curr_pass) {
            self.password = new_pass.to_owned();
            Ok(())
        } else {
            Err(anyhow!("incorrect password"))
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Post {
    id: u64,
    author_id: u64,
    contents: String,

    timestamp: u64,
}

impl Post {
    pub fn new(author: &User, contents: &str) -> Self {
        let now = Local::now().timestamp().try_into().unwrap();

        Self {
            id: 0,
            author_id: author.id,
            contents: contents.to_owned(),
            timestamp: now,
        }
    }
}

pub struct MemDb {
    next_user_id: u64,
    next_post_id: u64,
    users: Vec<User>,
    posts: Vec<Post>,
}

impl MemDb {
    pub fn new() -> Self {
        MemDb {
            next_user_id: 0,
            next_post_id: 0,
            users: Vec::new(),
            posts: Vec::new(),
        }
    }
}

impl AppDb for MemDb {
    fn num_users(&self) -> u64 {
        self.users.len() as u64
    }

    fn add_user(&mut self, mut user: User) -> Result<()> {
        // no dupes :)
        if self.users.iter().find(|x| x.name == user.name).is_some() {
            bail!("user with name {} already exists", user.name);
        }

        user.id = self.next_user_id;
        self.next_user_id += 1;

        self.users.push(user);
        Ok(())
    }

    fn update_user(&mut self, user: User) -> Result<()> {
        // verify that user already exists
        let user_ref = self
            .users
            .iter_mut()
            .find(|x| x.name == user.name)
            .ok_or(anyhow!("user with name {} not found", user.name))?;
        *user_ref = user;
        Ok(())
    }

    fn get_user_by_id(&self, id: u64) -> Result<User> {
        self.users
            .iter()
            .find(|x| x.id == id)
            .ok_or(anyhow!("user with id {} not found", id))
            .cloned()
    }

    fn get_user_by_name(&self, name: &str) -> Result<User> {
        self.users
            .iter()
            .find(|x| x.name == name)
            .ok_or(anyhow!("user with name {} not found", name))
            .cloned()
    }

    fn get_users(&self) -> Result<Vec<User>> {
        Ok(self.users.clone())
    }

    fn num_posts(&self) -> u64 {
        self.posts.len() as u64
    }

    fn add_post(&mut self, mut post: Post) -> Result<()> {
        post.id = self.next_post_id;
        self.next_post_id += 1;

        self.posts.push(post);
        Ok(())
    }

    fn update_post(&mut self, post: Post) -> Result<()> {
        let post_ref = self
            .posts
            .iter_mut()
            .find(|x| x.id == post.id)
            .ok_or(anyhow!("post with id {} not found", post.id))?;
        *post_ref = post;
        Ok(())
    }

    fn get_posts(&self) -> Result<Vec<Post>> {
        Ok(self.posts.clone())
    }

    fn delete_post_by_id(&mut self, id: u64) -> Result<()> {
        let idx = self
            .posts
            .iter()
            .position(|x| x.id == id)
            .ok_or(anyhow!("post with id {} not found", id))?;
        self.posts.swap_remove(idx);
        Ok(())
    }
}

pub trait AppDb {
    fn num_users(&self) -> u64;

    fn add_user(&mut self, user: User) -> Result<()>;

    fn update_user(&mut self, user: User) -> Result<()>;

    fn get_user_by_id(&self, id: u64) -> Result<User>;

    fn get_user_by_name(&self, name: &str) -> Result<User>;

    fn get_users(&self) -> Result<Vec<User>>;

    fn num_posts(&self) -> u64;

    fn add_post(&mut self, post: Post) -> Result<()>;

    fn update_post(&mut self, post: Post) -> Result<()>;

    fn get_posts(&self) -> Result<Vec<Post>>;

    fn delete_post_by_id(&mut self, id: u64) -> Result<()>;
}
