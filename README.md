# It's my website

Hey there! This is the source code for <https://kaylynn.gay/>. Go there first if you have no idea what this is.
It's comprised of a relatively simple backend that you might be interested in.

Past that, this is MIT licensed! Do with it what you will, though I'm not sure how much use you'll find it.

## Notes

Running this application is fairly straightforward, but you will want to set it up behind a web server (like Caddy or
nginx) and do a little bit of configuration.

The "content directory" can be found in this repository, under `website/content`. It contains the static content and
templates used by the application when rendering. You can put this directory wherever you want, but the application
needs to be configured so that it knows where it is. See the section on configuration for more details.

## Taking the easy way out

Alternatively: "just use Docker"

0.  Make sure your CWD is the repository root. `docker-bake.hcl`, `Dockerfile`, and this very `README.md` should be in
    your **current directory**. If they're not, it's time to whip out `cd` and fix that.

### First time setup

1.  Create a copy of `Caddyfile.example` named `Caddyfile` (without a file extension!) and replace the first line with
    the domain that the site will be hosted on.

    The compose file (and Caddyfile) assume that your working directory contains a provisioned SSL certificate; the
    certificate itself should reside in `cert.pem`, and the private key should live in `key.pem`.

    If you're using Tailscale, you can use a Tailscale domain name along with a Tailscale-provisioned HTTPS certificate
    to make things easier to run locally. `tailscale cert` is your friend.
2.  Run `docker volume create caddy_data` to create the (external) volume used by Caddy. **This volume is persistent**.
3.  Run `docker buildx bake` to build a docker image for the application. **You will have to re-run this step when
    pulling changes!**

### Running it

1.  Run `docker compose up --exit-code-from app` to start Caddy & the application.
2.  Leave it running until hell freezes over.
3.  Once everything has stopped, run `docker compose down -v` to clean up. It is **unfathomably important** that you
    pass the `-v` flag here, otherwise certain volumes created by the compose file will *not* be cleaned up, and you'll
    be left scratching your head when things seem out of date.

## Doing it the hard way

If you're not a Docker person, you have more work to do. Don't say I didn't warn you!

### Routing

This application assumes that it's running behind a web server. More specifically, routing should behave like so:

1.  Given a request to `path`, check if either `{path}` or `{path}/` exist  *relative to the static content directory*.
    If so, internally rewrite the URL to whichever path exists.
2.  Given a request to `path`, check the following *relative to the static content directory*:
    -   If `path` refers to a file, serve the file at `{static_content_directory}/{path}`.
    -   If `path` refers to a directory, check whether there is an `index.html` file in this directory. If so, strip the
        trailing slash from `path` and serve the file at `{static_content_directory}/{path}/index.html`.
3.  Otherwise, proxy any requests to the underlying application.

Assuming that:

1.  The web server is accessible at the domain `my.cool.domain.com`
2.  The application is running locally on port `8080`
3.  The static content directory is `my/static/content/directory`

... the following Caddyfile will be sufficient:

```caddy
my.cool.domain.com {
    root *  my/static/content/directory

    handle {
        try_files {path} {path}/
        file_server {
            pass_thru
        }
    }

    handle {
        reverse_proxy 127.0.0.1:8080
    }
}
```

If you're using nginx, you are A) not me and B) have bigger problems. Good luck!

### Configuration

This application uses [Figment](https://github.com/sergiobenitez/figment) for configuration, and pulls from the
following configuration sources, in order:

1.  An `App.toml` file. `App.toml` is looked up in the current working directory and each of its parent directories,
    terminating at the filesystem root. The first `App.toml` file to be found this way is used.
2.  Environment variables prefixed with `WOEBLOG_`.

When using `App.toml`, multiple "profiles" may be used. The general TOML schema for each profile is as follows:

```toml
[profile]
content_dir = "your/content/directory"
webhook_secret = "1234567890abcdef"

[profile.host]
address = "127.0.0.1"
port = 8080

```

The fields are as follows:
-   `profile.content_dir` is the directory that site content is located in.
-   `profile.webhook_secret` is the GitHub webhook secret, used with GHA to automatically deploy the application.
    **This value is optional**, and does not need to be specified.
-   `profile.host.address` is the address to bind to when running the application. This must be a valid IP address.
-   `profile.host.port` is the port to bind to when running the application. This value must be within the range of `0`
    and `65535`, inclusive.

`profile` may be any of `default`, `debug`, `release` or `global`. Debug builds of the application use the `debug`
profile, while release builds use the `release` profile.

Values in the `default` profile are used as a fallback if a value is not specified for the current profile.
Values in the `global` profile are used *regardless* of the current profile.

For example, the following configuration does the following:
-   Uses the same content directory regardless of profile
-   Uses a local address and port `8080` for the `debug` profile
-   Uses a webhook secret of "1234567890abcdef" for the `release` profile.
-   Exposes the application to the network and uses port `9090` for the `release` profile.

```toml
# Always use the same content directory
[default]
content_dir = "content"

# Run internally when debugging
[debug.host]
address = "127.0.0.1"
port = 8080

# Use a webhook secret for deployment in release mode
[release]
webhook_secret = "1234567890abcdef"

# Expose the application to the network in release mode, and use a different port
[release.host]
address = "0.0.0.0"
port = 9090
```

**Note that it is unwise to actually expose the application directly to the network like this**. See the above section
on routing for more details; you likely want to run the application without exposing it to the network, instead using a
web server (such as Caddy or nginx) to serve static files & reverse-proxy external traffic to the application.
