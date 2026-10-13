#!/usr/bin/env python3
"""Provision the FSP operations dashboard without overwriting operator edits."""
import json
from pathlib import Path
import shlex
import urllib.request

TITLE = "ФСП: состояние и диагностика"
API = "http://127.0.0.1:5080/api/default"


def axis(column: str) -> dict:
    return {"label": column, "alias": column, "column": column, "type": "raw", "color": None}


def panel(identifier: str, title: str, query: str, columns: list[str], position: int,
          *, stream: str = "fsp_logs", metric: bool = False, unit: str = "") -> dict:
    return {
        "id": identifier, "title": title, "description": "", "type": "line" if metric else "table",
        "config": {"show_legends": True, "unit": unit, "table_dynamic_columns": True},
        "queryType": "promql" if metric else "sql",
        "queries": [{
            "query": query, "customQuery": True,
            "fields": {
                "stream": stream, "stream_type": "metrics" if metric else "logs",
                "x": [axis(name) for name in columns], "y": [], "z": [],
                "filter": {"filterType": "group", "logicalOperator": "AND", "conditions": []},
            },
            "config": {"promql_legend": "{host}", "step_value": "30"},
        }],
        "layout": {"x": position % 2 * 96, "y": position // 2 * 15, "w": 96, "h": 15, "i": position + 1},
    }


def definition() -> dict:
    api = "service = 'fsp-api.service' AND event IN ('http_request', 'audit_command')"
    application = [
        panel("fsp-api-status", "API: запросы и ошибки", f"SELECT status, count(*) AS requests FROM fsp_logs WHERE {api} GROUP BY status ORDER BY status", ["status", "requests"], 0),
        panel("fsp-api-latency", "API: время ответа по маршрутам, мс", f"SELECT route, count(*) AS requests, avg(duration_ms) AS mean_ms, approx_percentile_cont(duration_ms, 0.95) AS p95_ms FROM fsp_logs WHERE {api} GROUP BY route ORDER BY p95_ms DESC LIMIT 30", ["route", "requests", "mean_ms", "p95_ms"], 1),
        panel("fsp-ai-latency", "ИИ: длительность и результат вызовов", "SELECT task, model, reasoning_effort, outcome, count(*) AS calls, avg(duration_ms) AS mean_ms FROM fsp_logs WHERE event = 'llm_call' GROUP BY task, model, reasoning_effort, outcome ORDER BY mean_ms DESC", ["task", "model", "reasoning_effort", "outcome", "calls", "mean_ms"], 2),
        panel("fsp-api-failures", "API: последние ошибки", f"SELECT _timestamp, route, method, status, code, request_id, trace_id FROM fsp_logs WHERE {api} AND status >= 400 ORDER BY _timestamp DESC LIMIT 100", ["_timestamp", "route", "method", "status", "code", "request_id", "trace_id"], 3),
    ]
    audit = [panel("fsp-audit", "Команды: субъект, действие, результат", "SELECT _timestamp, actor_id, role, route, method, status, code, request_id FROM fsp_logs WHERE event = 'audit_command' ORDER BY _timestamp DESC LIMIT 100", ["_timestamp", "actor_id", "role", "route", "method", "status", "code", "request_id"], 0)]
    audit[0]["layout"]["w"] = 192
    host = [
        panel("fsp-cpu", "CPU: занято, %", '100 * (1 - avg(rate(fsp_host_cpu_seconds_total{mode="idle"}[5m])))', [], 0, stream="fsp_host_cpu_seconds_total", metric=True, unit="percent"),
        panel("fsp-memory", "Память: доступно", "fsp_host_memory_available_bytes", [], 1, stream="fsp_host_memory_available_bytes", metric=True, unit="bytes"),
        panel("fsp-disk", "Диск: свободно", 'fsp_host_filesystem_free_bytes{mountpoint="/"}', [], 2, stream="fsp_host_filesystem_free_bytes", metric=True, unit="bytes"),
        panel("fsp-buffer", "Очередь логов: последнее измерение", "SELECT _timestamp, component_id, buffer_type, value AS bytes FROM fsp_collector_buffer_size_bytes WHERE component_id = 'openobserve' ORDER BY _timestamp DESC LIMIT 1", ["_timestamp", "component_id", "buffer_type", "bytes"], 3, stream="fsp_collector_buffer_size_bytes"),
    ]
    host[3]["queries"][0]["fields"]["stream_type"] = "metrics"
    return {
        "version": 8, "title": TITLE,
        "description": "Диагностика ФСП. Аудит содержит внутренние идентификаторы; содержимое профилей и ответов не собирается. Сессии и записи доступны в разделе RUM.",
        "tabs": [
            {"tabId": "application", "name": "API и ИИ", "panels": application},
            {"tabId": "audit", "name": "Аудит", "panels": audit},
            {"tabId": "host", "name": "Сервер и сборщик", "panels": host},
        ],
        "defaultDatetimeDuration": {"type": "relative", "relativeTimePeriod": "1h"},
        "variables": {"list": [], "showDynamicFilters": False},
    }


def request(authorization: str, path: str, payload: dict | None = None) -> dict:
    req = urllib.request.Request(
        API + path, data=json.dumps(payload).encode() if payload is not None else None,
        headers={"Authorization": authorization, "Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=30) as response:
        return json.load(response)


def provision_dashboard(authorization: str) -> str:
    existing = [item for item in request(authorization, "/dashboards")["dashboards"] if item["title"] == TITLE]
    if len(existing) > 1:
        raise RuntimeError("Multiple FSP operations dashboards exist; reconcile explicitly")
    if existing:
        return existing[0]["dashboard_id"]
    saved = request(authorization, "/dashboards", definition())
    return saved["v8"]["dashboardId"]


if __name__ == "__main__":
    values = dict(shlex.split(line)[0].split("=", 1) for line in Path("/etc/fsp/observability.env").read_text().splitlines() if line and not line.startswith("#"))
    print(json.dumps({"dashboard_id": provision_dashboard(values["FSP_OBSERVE_AUTH"])}))
