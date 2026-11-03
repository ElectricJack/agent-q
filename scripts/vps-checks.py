#!/usr/bin/env python3
"""Live E5 checks of the VPS boundary, printed as JSON evidence.

  origin   TCP-probe the VPS public address from OUTSIDE the VPS: only the
           allowed ports (SSH) may accept a connection.
  machine  The machine hostname: without a service token Access refuses
           every request; with one, /healthz answers and every human route
           is 404 (the worker listener has no human routes).
  human    The human hostname: without a login Access refuses or redirects
           to its login page; a service token is not accepted either.

Service token: CF_ACCESS_CLIENT_ID and CF_ACCESS_CLIENT_SECRET.
Exit 0 when every requested check passes, 1 when one fails, 2 on usage errors.
"""
import argparse
import json
import os
import socket
import sys
import urllib.error
import urllib.request

DEFAULT_PORTS = "22,80,443,2375,2376,3000,5432,8080,8081,9090"
HUMAN_ROUTES = ["/", "/api/me", "/api/status", "/api/workers", "/api/users", "/api/updates"]
REFUSED = (401, 403)


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


OPENER = urllib.request.build_opener(NoRedirect)


def get(url, headers, timeout):
    """Status code and Location of one GET, without following redirects."""
    request = urllib.request.Request(url, headers={"User-Agent": "aq-vps-checks", **headers})
    try:
        with OPENER.open(request, timeout=timeout) as response:
            return response.status, response.headers.get("Location", "")
    except urllib.error.HTTPError as error:
        return error.code, error.headers.get("Location", "")
    except (urllib.error.URLError, OSError) as error:
        return f"error: {error}", ""


def access_refused(status, location):
    """Access refused the request before it reached the origin."""
    return status in REFUSED or (status in (302, 303) and "cloudflareaccess.com" in location)


def token_headers():
    client_id = os.environ.get("CF_ACCESS_CLIENT_ID")
    secret = os.environ.get("CF_ACCESS_CLIENT_SECRET")
    if not client_id or not secret:
        sys.exit("set CF_ACCESS_CLIENT_ID and CF_ACCESS_CLIENT_SECRET to a machine service token")
    return {"CF-Access-Client-Id": client_id, "CF-Access-Client-Secret": secret}


def check_origin(args):
    ports = [int(p) for p in args.ports.split(",") if p]
    allowed = {int(p) for p in args.allow.split(",") if p}
    accepting = []
    for port in ports:
        try:
            with socket.create_connection((args.host, port), timeout=args.timeout):
                accepting.append(port)
        except OSError:
            pass
    unexpected = [p for p in accepting if p not in allowed]
    return not unexpected, {"host": args.host, "probed": ports, "allowed": sorted(allowed),
                            "accepting": accepting, "open": unexpected}


def probe(base, paths, headers, timeout):
    return {p: get(base.rstrip("/") + p, headers, timeout) for p in paths}


def check_machine(args):
    token = token_headers()
    paths = ["/healthz", *HUMAN_ROUTES]
    bare = probe(args.url, paths, {}, args.timeout)
    authed = probe(args.url, paths, token, args.timeout)
    ok = (all(access_refused(*bare[p]) for p in paths)
          and authed["/healthz"][0] == 200
          and all(authed[r][0] == 404 for r in HUMAN_ROUTES))
    return ok, {"url": args.url, "human_routes": HUMAN_ROUTES,
                "without_token": {p: s for p, (s, _) in bare.items()},
                "with_token": {p: s for p, (s, _) in authed.items()}}


def check_human(args):
    paths = ["/", "/api/me", "/healthz"]
    bare = probe(args.url, paths, {}, args.timeout)
    token = probe(args.url, paths, token_headers(), args.timeout)
    ok = all(access_refused(*bare[p]) for p in paths) and all(access_refused(*token[p]) for p in paths)
    return ok, {"url": args.url,
                "without_login": {p: {"status": s, "location": l} for p, (s, l) in bare.items()},
                "with_service_token": {p: s for p, (s, _) in token.items()}}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--timeout", type=float, default=5.0)
    sub = parser.add_subparsers(dest="check", required=True)
    origin = sub.add_parser("origin")
    origin.add_argument("--host", required=True)
    origin.add_argument("--ports", default=DEFAULT_PORTS)
    origin.add_argument("--allow", default="22")
    for name in ("machine", "human"):
        sub.add_parser(name).add_argument("--url", required=True)
    args = parser.parse_args()
    ok, detail = {"origin": check_origin, "machine": check_machine, "human": check_human}[args.check](args)
    print(json.dumps({"ok": ok, args.check: detail}, indent=2))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
