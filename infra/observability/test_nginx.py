import unittest
from pathlib import Path

from nginx import render


class IntakeConfigurationTests(unittest.TestCase):
    def test_intake_has_no_generic_proxy_and_discards_browser_headers(self):
        http, locations = render("https://nddev.link", "synthetic-token", Path(__file__).resolve().parents[2] / "contracts/openapi.json")
        self.assertEqual(locations.count("proxy_pass http://127.0.0.1:5080/rum/v1/default/"), 2)
        self.assertEqual(locations.count("proxy_pass_request_headers off"), 2)
        self.assertEqual(locations.count('if ($http_origin != "https://nddev.link")'), 2)
        self.assertIn('location ^~ /observe/ { return 404; }', locations)
        for private in ("$request_uri", "$http_cookie", "$http_authorization", "$args", "$remote_addr", "$http_referer"):
            self.assertNotIn(private, http)
        self.assertIn('/api/v1/candidates/:id', http)

    def test_interpolated_values_cannot_inject_nginx_directives(self):
        contract = Path(__file__).resolve().parents[2] / "contracts/openapi.json"
        for origin, token in [('https://nddev.link;include evil;', 'safe'), ('https://nddev.link', '"; include evil;')]:
            with self.assertRaises(ValueError):
                render(origin, token, contract)


if __name__ == '__main__':
    unittest.main()
