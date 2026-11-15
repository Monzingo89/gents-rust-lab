"""Versioned address admission, independent of HTTP, credentials and models."""
import ipaddress
import json
from pathlib import Path
import socket


class PublicAddressPolicy:
    def __init__(self, value):
        if type(value["version"]) is not int or value["version"] != 1 or value["scheme"] != "https" or type(value["port"]) is not int or value["port"] != 443:
            raise ValueError("unsupported public network policy")
        self.version = value["version"]
        self.ipv6_allowed = self._networks(value["ipv6_allowed"], 6)
        self.denied = {4: self._networks(value["deny_ipv4"], 4),
                       6: self._networks(value["deny_ipv6"], 6)}
        if not self.ipv6_allowed or not all(self.denied.values()):
            raise ValueError("incomplete public network policy")

    @staticmethod
    def _networks(values, family):
        if not isinstance(values, list) or not 1 <= len(values) <= 128:
            raise ValueError("bounded address prefixes required")
        if any(not isinstance(value, str) or "/" not in value for value in values):
            raise ValueError("explicit address prefixes required")
        networks = tuple(ipaddress.ip_network(value, strict=True) for value in values)
        if any(network.version != family for network in networks):
            raise ValueError("address prefix family mismatch")
        return networks

    def allows(self, address):
        if not isinstance(address, str) or "%" in address:
            return False
        try:
            ip = ipaddress.ip_address(address)
        except ValueError:
            return False
        return ((ip.version == 4 or any(ip in n for n in self.ipv6_allowed))
                and not any(ip in n for n in self.denied[ip.version]))


POLICY = PublicAddressPolicy(json.loads((Path(__file__).resolve().parents[1]
                                       / "config/public-network-policy.json").read_text()))


def public_address(host):
    if not isinstance(host, str) or not host or "%" in host:
        raise ValueError("unscoped public hostname required")
    answers = socket.getaddrinfo(host, 443, type=socket.SOCK_STREAM)
    def valid(row):
        address = row[4]
        family = {socket.AF_INET: 4, socket.AF_INET6: 6}.get(row[0])
        return (POLICY.allows(address[0]) and ipaddress.ip_address(address[0]).version == family
                and (len(address) < 4 or address[3] == 0))
    if not 1 <= len(answers) <= 64 or any(not valid(row) for row in answers):
        raise ValueError("web tools require public internet hosts, not local/private or translated addresses")
    return answers[0][4][0]
