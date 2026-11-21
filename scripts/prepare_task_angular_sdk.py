#!/usr/bin/env python3
"""Explicit development SDK preparation; never invoked by a task or MCP tool.

Downloads public SDK dependencies only with --install. No lifecycle scripts,
user npm credentials, model downloads, global installs or service activation.
"""

import argparse
import hashlib
import json
import os
import shutil
import signal
import subprocess
import time
from pathlib import Path

ANGULAR = "21.2.26"
MANIFEST = {
    "name": "gents-isolated-angular-sdk",
    "private": True,
    "version": "1.0.0",
    "type": "module",
    "dependencies": {
        **{
            name: ANGULAR
            for name in [
                "@angular/build",
                "@angular/cli",
                "@angular/common",
                "@angular/compiler",
                "@angular/compiler-cli",
                "@angular/core",
                "@angular/platform-browser",
            ]
        },
        "rxjs": "7.8.2",
        "tslib": "2.8.1",
        "typescript": "5.9.3",
    },
}
FILES = {
    "angular.json": json.dumps(
        {
            "version": 1,
            "cli": {"analytics": False, "cache": {"enabled": False}},
            "projects": {
                "fixture": {
                    "projectType": "application",
                    "root": "",
                    "sourceRoot": "src",
                    "architect": {
                        "build": {
                            "builder": "@angular/build:application",
                            "options": {
                                "browser": "src/main.ts",
                                "index": "src/index.html",
                                "outputPath": "dist",
                                "tsConfig": "tsconfig.app.json",
                                "assets": [],
                                "styles": [],
                                "polyfills": [],
                            },
                            "configurations": {
                                "development": {
                                    "optimization": False,
                                    "sourceMap": False,
                                    "extractLicenses": False,
                                }
                            },
                        }
                    },
                }
            },
        },
        indent=2,
    )
    + "\n",
    "tsconfig.json": json.dumps(
        {
            "compilerOptions": {
                "target": "ES2022",
                "module": "ES2022",
                "moduleResolution": "bundler",
                "experimentalDecorators": True,
                "strict": True,
                "skipLibCheck": True,
                "importHelpers": True,
                "lib": ["ES2022", "DOM"],
                "types": [],
            },
            "angularCompilerOptions": {"strictTemplates": True},
        },
        indent=2,
    )
    + "\n",
    "tsconfig.app.json": json.dumps(
        {
            "extends": "./tsconfig.json",
            "compilerOptions": {"outDir": ".types"},
            "files": ["src/main.ts"],
        },
        indent=2,
    )
    + "\n",
    "src/index.html": '<!doctype html><html><head><meta charset="utf-8"><base href="/"><title>Angular validation fixture</title></head><body><app-root></app-root></body></html>\n',
    "src/main.ts": "import {Component, signal} from '@angular/core';\nimport {bootstrapApplication} from '@angular/platform-browser';\n@Component({selector:'app-root',standalone:true,template:'<h1>Angular fixture</h1><button id=\"increment\" (click)=\"count.set(count()+1)\">Add</button><output id=\"value\">{{count()}}</output>'})\nclass App { readonly count=signal(0); }\nbootstrapApplication(App).catch(error=>console.error(error));\n",
}


def write_owned(path, text):
    if path.exists():
        if path.read_text() != text:
            raise RuntimeError(
                f"Refusing to overwrite a changed SDK fixture: {path.name}"
            )
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--sdk-dir",
        type=Path,
        default=Path.home() / "Library/Caches/gents-sdk/angular-21.2.26",
    )
    parser.add_argument("--node", type=Path, default=Path("/opt/homebrew/bin/node"))
    parser.add_argument("--install", action="store_true")
    args = parser.parse_args()
    sdk = args.sdk_dir.expanduser().resolve()
    node = args.node.resolve(strict=True)
    if not args.install:
        print(
            json.dumps(
                {
                    "sdk": str(sdk),
                    "cli_present": (
                        sdk / "node_modules/@angular/cli/package.json"
                    ).is_file(),
                    "build_present": (
                        sdk / "node_modules/@angular/build/package.json"
                    ).is_file(),
                    "installed_or_executed": False,
                }
            )
        )
        return
    sdk.mkdir(parents=True, exist_ok=True)
    if (sdk / ".npmrc").exists():
        raise RuntimeError("SDK project .npmrc is not permitted; no install attempted")
    existing = sdk / "package.json"
    if existing.exists():
        prior = json.loads(existing.read_text())
        initial = {
            "name": "gents-isolated-angular-sdk",
            "private": True,
            "version": "1.0.0",
            "dependencies": {"@angular/cli": ANGULAR},
        }
        if prior not in [initial, MANIFEST]:
            raise RuntimeError(
                "SDK directory is not the owned fixture; no files changed"
            )
    existing.write_text(json.dumps(MANIFEST, indent=2) + "\n")
    for name, text in FILES.items():
        write_owned(sdk / name, text)
    for name in [".npm-user-empty", ".npm-global-empty"]:
        write_owned(sdk / name, "")
    env = {
        "PATH": str(node.parent) + ":/usr/bin:/bin",
        "LC_ALL": "C",
        "CI": "1",
        "NG_CLI_ANALYTICS": "false",
        "NODE_OPTIONS": "--max-old-space-size=1024",
        "NPM_CONFIG_USERCONFIG": str(sdk / ".npm-user-empty"),
        "NPM_CONFIG_GLOBALCONFIG": str(sdk / ".npm-global-empty"),
        "NPM_CONFIG_REGISTRY": "https://registry.npmjs.org",
        "NPM_CONFIG_CACHE": str(sdk / ".npm-cache"),
        "NPM_CONFIG_AUDIT": "false",
        "NPM_CONFIG_FUND": "false",
        "NPM_CONFIG_UPDATE_NOTIFIER": "false",
        "NPM_CONFIG_MAXSOCKETS": "4",
        "NPM_CONFIG_FETCH_RETRIES": "1",
    }
    if os.environ.get("TMPDIR"):
        env["TMPDIR"] = os.environ["TMPDIR"]
    npm = Path(shutil.which("npm") or "").resolve(strict=True)
    started = time.monotonic()
    log = sdk / "sdk-prepare.log"
    with log.open("w") as output:
        process = subprocess.Popen(
            [
                str(node),
                str(npm),
                "install",
                "--ignore-scripts",
                "--no-audit",
                "--no-fund",
            ],
            cwd=sdk,
            env=env,
            stdout=output,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        try:
            code = process.wait(timeout=300)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
            raise RuntimeError(
                "SDK preparation exceeded its finite setup deadline; inspect sdk-prepare.log"
            ) from None
    if code:
        raise RuntimeError(f"SDK preparation exited {code}; inspect sdk-prepare.log")
    versions = {
        name: json.loads((sdk / "node_modules" / name / "package.json").read_text())[
            "version"
        ]
        for name in MANIFEST["dependencies"]
    }
    assert versions == MANIFEST["dependencies"], versions
    report = {
        "schema_version": 1,
        "state": "installed_not_build_or_browser_verified",
        "sdk": str(sdk),
        "node": str(node),
        "node_sha256": hashlib.sha256(node.read_bytes()).hexdigest(),
        "node_version": subprocess.check_output(
            [str(node), "--version"], env=env, text=True
        ).strip(),
        "versions": versions,
        "package_lock_sha256": hashlib.sha256(
            (sdk / "package-lock.json").read_bytes()
        ).hexdigest(),
        "elapsed_seconds": round(time.monotonic() - started, 3),
        "lifecycle_scripts_run": False,
        "model_downloads": 0,
        "provider_cost_usd": 0,
    }
    (sdk / "sdk-prepared.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
