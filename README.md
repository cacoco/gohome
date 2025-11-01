# gohome

A Rust-powered port of the Tailscale [golink](https://github.com/tailscale/golink) application but without Tailscale. This is for running on a personal home network that does not use Tailscale. Thus there is no true user context for links.

Another big difference, since this is written in Rust, it uses [handlebars-rust](https://github.com/sunng87/handlebars-rust) instead of the Go [text/template](https://pkg.go.dev/text/template) library for advanced URL path features.

## Why

We currently use the Tailscale [golink](https://github.com/tailscale/golink) application at my office and it is great. I'd love to have a similar set up on my home network where I use a [Firewalla Gold Pro](https://firewalla.com/products/firewalla-gold-pro) running a [WireGuard VPN server](https://help.firewalla.com/hc/en-us/articles/1500004087521-WireGuard-VPN-Server-Configuration).

The project is purposely meant to resemble the original Tailscale golink application to be somewhat of a seemless transition.

### Why is this in Rust?

A personal challenge to develop Rust experience.

## Known Issues

Currently the XSRF token isn't properly generated.

## Disclaimer

This project is not associated with Tailscale Inc.
