#!/usr/bin/env python3
"""Exercise a temporary loopback nginx with the exact intake renderer; never reload production."""
import json
from pathlib import Path
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

from install import read_env
from nginx import render


def main() -> None:
    here = Path(__file__).resolve().parent
    values = read_env(Path('/etc/fsp/observability.env'))
    request = urllib.request.Request('http://127.0.0.1:5080/api/default/rumtoken', headers={'Authorization': values['FSP_OBSERVE_AUTH']})
    with urllib.request.urlopen(request, timeout=10) as response:
        token = json.load(response)['data']['rum_token']
    contract = here.parent.parent / 'contracts/openapi.json'
    if not contract.exists():
        contract = here / 'openapi.json'
    public_url = read_env(Path('/etc/fsp/rust-api.env'))['FSP_PUBLIC_WEB_URL']
    http, locations = render(public_url, token, contract)
    with tempfile.TemporaryDirectory(prefix='fsp-intake-check-') as temporary:
        directory = Path(temporary)
        access_log = directory / 'access.log'
        locations = locations.replace('include snippets/', 'include /etc/nginx/snippets/').replace('/var/log/nginx/fsp-telemetry.json', str(access_log))
        config = directory / 'nginx.conf'
        config.write_text(f'pid {directory}/nginx.pid;\nerror_log {directory}/error.log warn;\nevents {{}}\nhttp {{\nmap $scheme $fsp_hsts {{ default ""; }}\n{http}\nserver {{ listen 127.0.0.1:5090;\n{locations}\n}}\n}}\n')
        config.chmod(0o600)
        subprocess.run(['nginx', '-t', '-c', str(config)], check=True)
        process = subprocess.Popen(['nginx', '-c', str(config), '-g', 'daemon off;'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            time.sleep(1)
            cases = [('GET', '/observe/rum', None, 405), ('POST', '/observe/rum', 'https://evil.example', 403), ('POST', '/observe/rum?token=private-canary', public_url, 400), ('POST', '/observe/admin', public_url, 404), ('POST', '/observe/rum', public_url, 202)]
            for method, path, origin, expected in cases:
                headers = {'Content-Type': 'application/json', 'Cookie': 'private-canary-cookie', 'Authorization': 'Bearer private-canary-token'}
                if origin:
                    headers['Origin'] = origin
                body = json.dumps({'type': 'view', 'service': 'fsp-ingress-probe', 'application': {'id': 'fsp-career'}, 'date': int(time.time() * 1000)}).encode()
                request = urllib.request.Request('http://127.0.0.1:5090' + path, data=body if method == 'POST' else None, headers=headers, method=method)
                try:
                    with urllib.request.urlopen(request, timeout=10) as response:
                        status = response.status
                except urllib.error.HTTPError as error:
                    status = error.code
                if status != expected:
                    raise SystemExit(f'Intake boundary returned {status}, expected {expected}')
        finally:
            process.terminate()
            process.wait(timeout=10)
        if 'private-canary' in access_log.read_text():
            raise SystemExit('Private canary leaked into nginx telemetry')
        print('Intake: five real HTTP boundary checks passed; access-log canaries removed')


if __name__ == '__main__':
    main()
