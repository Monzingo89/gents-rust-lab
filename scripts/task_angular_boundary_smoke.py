#!/usr/bin/env python3
"""Exercise the actual checked-in browser Seatbelt profile with native probes.

Only temporary de-identified sentinels and host-held loopback sockets are used.
No browser/model install, credentials, background service or paid provider.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile

SOURCE = r'''
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/socket.h>
#include <sys/sysctl.h>
#include <sys/un.h>
#include <sys/wait.h>
#include <unistd.h>
static bool denied_open(const char *path,int flags) {
  errno=0;int fd=open(path,flags,0600);bool denied=fd<0&&(errno==EPERM||errno==EACCES);if(fd>=0)close(fd);return denied;
}
static int connect_port(int port,const char *address) {
  int fd=socket(AF_INET,SOCK_STREAM,0);if(fd<0)return -2;
  fcntl(fd,F_SETFL,O_NONBLOCK);struct sockaddr_in target={0};target.sin_family=AF_INET;target.sin_port=htons(port);inet_pton(AF_INET,address,&target.sin_addr);
  int result=connect(fd,(struct sockaddr *)&target,sizeof(target));int error=errno;close(fd);return result==0||error==EINPROGRESS?0:(error==EPERM||error==EACCES?1:2);
}
int main(int argc,char **argv) {
  if(argc!=7)return 2;
  bool read_denied=denied_open(argv[1],O_RDONLY),write_denied=denied_open(argv[1],O_WRONLY),oracle_denied=denied_open(argv[2],O_RDONLY);
  int fd=open(argv[3],O_CREAT|O_RDWR,0600);bool profile_write=fd>=0;if(fd>=0)close(fd);
  bool asset_port=connect_port(atoi(argv[4]),"127.0.0.1")==0;
  bool unrelated_port=connect_port(atoi(argv[5]),"127.0.0.1")==1;
  bool external=connect_port(80,"192.0.2.1")==1;
  int tcp=socket(AF_INET,SOCK_STREAM,0);struct sockaddr_in local={0};local.sin_family=AF_INET;local.sin_addr.s_addr=htonl(INADDR_LOOPBACK);errno=0;
  bool bind_denied=tcp>=0&&bind(tcp,(struct sockaddr*)&local,sizeof(local))<0&&(errno==EPERM||errno==EACCES);if(tcp>=0)close(tcp);
  int mib[]={CTL_KERN,KERN_PROCARGS2,atoi(argv[6])};char args[4096];size_t size=sizeof(args);errno=0;
  bool argv_denied=sysctl(mib,3,args,&size,NULL,0)<0&&(errno==EPERM||errno==EACCES);
  pid_t child=fork();if(child==0){execl("/bin/sh","sh","-c","exit 0",NULL);_exit(errno==EPERM||errno==EACCES?77:78);}int status=0;bool exec_denied=child>0&&waitpid(child,&status,0)==child&&WIFEXITED(status)&&WEXITSTATUS(status)==77;
  printf("{\"outside_read_denied\":%s,\"outside_write_denied\":%s,\"oracle_read_denied\":%s,\"profile_write_allowed\":%s,\"asset_port_allowed\":%s,\"unrelated_loopback_denied\":%s,\"external_connect_denied\":%s,\"new_tcp_bind_denied\":%s,\"parent_argv_denied\":%s,\"shell_exec_denied\":%s}\n",read_denied?"true":"false",write_denied?"true":"false",oracle_denied?"true":"false",profile_write?"true":"false",asset_port?"true":"false",unrelated_port?"true":"false",external?"true":"false",bind_denied?"true":"false",argv_denied?"true":"false",exec_denied?"true":"false");
  return read_denied&&write_denied&&oracle_denied&&profile_write&&asset_port&&unrelated_port&&external&&bind_denied&&argv_denied&&exec_denied?0:1;
}
'''


def bounded(command, **kwargs):
    process = subprocess.Popen(command, start_new_session=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, **kwargs)
    try:
        stdout, stderr = process.communicate(timeout=20)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.communicate()
        raise RuntimeError("finite boundary probe deadline exceeded") from None
    finally:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    if process.returncode:
        raise RuntimeError(f"boundary probe exit {process.returncode}; stdout={stdout[:2000]!r}; stderr={stderr[:1000]!r}")
    return stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--node", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    profile_source = root/"mcp/src/angular_check/browser_profile.mjs"
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "OPENSSL_CONF": "/dev/null"}
    with tempfile.TemporaryDirectory(prefix="gents-angular-boundary-") as directory, socket.socket() as assets, socket.socket() as unrelated:
        control = Path(directory).resolve()
        sdk = control/"sdk"
        sdk.mkdir()
        profile = control/"profile"
        profile.mkdir()
        sentinel = control/"outside-sentinel"
        sentinel.write_text("de-identified sentinel")
        oracle = control/"oracle-private"
        oracle.write_text("owner expected values stay outside browser authority")
        source = sdk/"probe.c"
        source.write_text(SOURCE)
        binary = sdk/"chrome-headless-shell"
        bounded(["/usr/bin/clang", "-Wall", "-Werror", str(source), "-o", str(binary)], env=env)
        assets.bind(("127.0.0.1", 0))
        assets.listen(2)
        unrelated.bind(("127.0.0.1", 0))
        unrelated.listen(2)
        port = assets.getsockname()[1]
        code = f"import {{browserProfile}} from {json.dumps(profile_source.as_uri())};process.stdout.write(browserProfile(...JSON.parse(process.argv[1])));"
        actual_profile = bounded([str(args.node.resolve(strict=True)), "--input-type=module", "-e", code, json.dumps([str(binary), str(profile), port])], env=env).decode()
        observed = json.loads(bounded(["/usr/bin/sandbox-exec", "-p", actual_profile, str(binary), str(sentinel), str(oracle), str(profile/"allowed-file"), str(port), str(unrelated.getsockname()[1]), str(os.getpid())], env=env))
        assert all(observed.values()) and len(observed) == 10, observed
        assert sentinel.read_text() == "de-identified sentinel"
        report = {"schema_version": 1, "scope": "Native OS probes against actual checked-in browser profile, independent of Playwright routing/CSP; all fixtures de-identified", "profile_source_sha256": hashlib.sha256(profile_source.read_bytes()).hexdigest(), "profile_sha256": hashlib.sha256(actual_profile.encode()).hexdigest(), "checks": observed, "provider_cost_usd": 0, "model_calls": 0}
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2)+"\n")
        print(json.dumps(report))


if __name__ == "__main__":
    main()
