"""Offline admission and mocked-transport proofs; never contact private services."""
import io
import json
from pathlib import Path
import shutil
import socket
import ssl
import subprocess
import unittest
from unittest.mock import Mock, patch
import company_tools as tools
import public_network as network


class PublicNetworkTests(unittest.TestCase):
    def test_shared_addresses_and_javascript_concordance(self):
        fixture=Path(__file__).parent/"fixtures/public-addresses.json"
        rows=json.loads(fixture.read_text())
        for row in rows:
            self.assertEqual(network.POLICY.allows(row["address"]),row["allowed"],row["address"])
        node=shutil.which("node")
        self.assertIsNotNone(node,"cross-runtime security verification requires Node")
        code="import {publicAddress} from './scripts/public-network.mjs';import {readFileSync} from 'node:fs';const rows=JSON.parse(readFileSync('./scripts/fixtures/public-addresses.json','utf8'));console.log(JSON.stringify(rows.map(r=>publicAddress(r.address))));"
        actual=json.loads(subprocess.check_output([node,"--input-type=module","-e",code],cwd=Path(__file__).resolve().parents[1]))
        self.assertEqual(actual,[network.POLICY.allows(row["address"]) for row in rows])

    def test_mixed_dns_and_translation_targets_fail_before_socket_creation(self):
        for address in ["127.0.0.1","::ffff:127.0.0.1","64:ff9b::7f00:1","2002:7f00:1::"]:
            answers=[(socket.AF_INET,socket.SOCK_STREAM,6,"",("1.1.1.1",443)),(socket.AF_INET6 if ":" in address else socket.AF_INET,socket.SOCK_STREAM,6,"",(address,443))]
            with patch.object(network.socket,"getaddrinfo",return_value=answers),patch.object(tools.socket,"create_connection") as connect:
                with self.assertRaises(ValueError):tools.PinnedHTTPS("fixture.invalid",context=tools.tls_context()).connect()
                connect.assert_not_called()

    def test_connection_uses_validated_numeric_address_and_original_tls_name(self):
        answers=[(socket.AF_INET,socket.SOCK_STREAM,6,"",("1.1.1.1",443))]
        context=Mock();sock=Mock()
        with patch.object(network.socket,"getaddrinfo",return_value=answers) as resolve,patch.object(tools.socket,"create_connection",return_value=sock) as connect:
            connection=tools.PinnedHTTPS("public.invalid",context=context);connection.connect()
            self.assertEqual(resolve.call_count,1)
            self.assertEqual(connect.call_args.args[0],("1.1.1.1",443))
            context.wrap_socket.assert_called_once_with(sock,server_hostname="public.invalid")
        actual=tools.tls_context()
        self.assertTrue(actual.check_hostname);self.assertEqual(actual.verify_mode,ssl.CERT_REQUIRED)

    def test_each_redirect_is_revalidated_and_caller_headers_cannot_follow(self):
        class Socket:
            def __init__(self):self.sent=[]
            def sendall(self,data):self.sent.append(data)
            def makefile(self,*args):return io.BytesIO(b"HTTP/1.1 302 Found\r\nLocation: https://private.invalid/\r\nContent-Length: 0\r\n\r\n")
            def close(self):pass
        def dns(host,*args,**kwargs):return [(socket.AF_INET,socket.SOCK_STREAM,6,"",("127.0.0.1" if host=="private.invalid" else "1.1.1.1",443))]
        for headers in [None,{"authorization":"fixture-secret"},{"AUTHORIZATION":"fixture-secret"},{"Cookie":"fixture-cookie"}]:
            context=Mock();context.wrap_socket.side_effect=lambda sock,**kw:sock
            with patch.object(network.socket,"getaddrinfo",side_effect=dns),patch.object(tools,"tls_context",return_value=context),patch.object(tools.socket,"create_connection",side_effect=lambda *a,**kw:Socket()) as connect:
                with self.assertRaises(ValueError):tools.web_request("https://public.invalid/",headers=headers)
                self.assertEqual(connect.call_count,1,"private redirect must never open a second socket")

    def test_security_adapters_and_policy_are_not_agent_repair_targets(self):
        for name in ["scripts/company-browser.mjs","scripts/public-network.mjs","scripts/public_network.py","scripts/browser-public-http.mjs","config/public-network-policy.json"]:
            with self.assertRaisesRegex(ValueError,"human review"):tools.source_target(name)


if __name__=="__main__":unittest.main()
