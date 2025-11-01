# gohome

A Rust-powered port of the Tailscale [golink](https://github.com/tailscale/golink) application but without Tailscale. This is for running on a personal home network that does not use Tailscale. Thus there is no true user context for links.

## Why

We currently use the Tailscale [golink](https://github.com/tailscale/golink) application at my office and it is great. I'd love to have a similar set up on my home network where I use a [Firewalla Gold Pro](https://firewalla.com/products/firewalla-gold-pro) running a [WireGuard VPN server](https://help.firewalla.com/hc/en-us/articles/1500004087521-WireGuard-VPN-Server-Configuration).

The project is purposely meant to resemble the original Tailscale golink application to be somewhat of a seemless transition.

### Why is this in Rust?

A personal challenge to develop Rust experience.

## Disclaimer

This project is not associated with Tailscale Inc.
