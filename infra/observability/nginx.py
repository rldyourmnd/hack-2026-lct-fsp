"""Render two fixed same-origin intake routes and an allowlisted access log."""
from __future__ import annotations

import json
from pathlib import Path
import re
import urllib.parse


def render(public_url: str, rum_token: str, contract: Path) -> tuple[str, str]:
    parsed = urllib.parse.urlsplit(public_url)
    if parsed.scheme != "https" or not re.fullmatch(r"[a-z0-9.-]+", parsed.netloc) or parsed.path not in ("", "/") or parsed.query or parsed.fragment:
        raise ValueError("Expected a canonical HTTPS origin")
    if not re.fullmatch(r"[A-Za-z0-9_=-]+", rum_token):
        raise ValueError("Unexpected RUM credential encoding")
    paths = json.loads(contract.read_text())["paths"]
    routes = ['map $uri $fsp_observe_route {', '    default "unmatched";']
    for path in sorted(paths, key=lambda path: (-len(path), path)):
        pattern = re.sub(r"\{[^}]+\}", "[^/]+", path)
        label = re.sub(r"\{[^}]+\}", ":id", path)
        routes.append(f'    ~^{pattern}$ "{label}";')
    routes.extend(['    ~^/assets/ "/assets/:file";', '    ~^/candidate/ "/candidate/:page";', '    ~^/employer/ "/employer/:page";', '    /login "/login";', '}'])
    http = "\n".join(routes) + r'''
map $request_method $fsp_observe_method {
    default "other";
    GET GET; POST POST; PUT PUT; PATCH PATCH; DELETE DELETE; OPTIONS OPTIONS; HEAD HEAD;
}
limit_req_zone $binary_remote_addr zone=fsp_observe:10m rate=10r/s;
log_format fsp_observe escape=json '{"route":"$fsp_observe_route","method":"$fsp_observe_method","status":$status,"duration_seconds":$request_time,"request_id":"$request_id","bytes":$body_bytes_sent}';
'''
    locations = ['access_log /var/log/nginx/fsp-telemetry.json fsp_observe;']
    for endpoint in ("rum", "replay"):
        locations.append(f'''
location = /observe/{endpoint} {{
    if ($request_method != POST) {{ return 405; }}
    if ($http_origin != "{public_url.rstrip('/')}") {{ return 403; }}
    if ($args != "") {{ return 400; }}
    client_max_body_size 1m;
    limit_req zone=fsp_observe burst=30 nodelay;
    limit_req_status 429;
    proxy_pass http://127.0.0.1:5080/rum/v1/default/{endpoint};
    proxy_pass_request_headers off;
    proxy_set_header Host 127.0.0.1:5080;
    proxy_set_header Content-Type $http_content_type;
    proxy_set_header oo-api-key "{rum_token}";
    proxy_set_header User-Agent FSP-RUM;
    proxy_hide_header Set-Cookie;
    proxy_connect_timeout 2s;
    proxy_read_timeout 10s;
    proxy_send_timeout 10s;
    add_header Cache-Control "no-store" always;
    include snippets/fsp-security-headers.conf;
}}
''')
    locations.append('location ^~ /observe/ { return 404; }\n')
    return http, "\n".join(locations)
