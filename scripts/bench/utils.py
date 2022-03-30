import subprocess as sp
import json

TOOLCHAIN = 'nightly-2022-02-17'

def get_cargo_cmd(cmd: str, driver_pattern: str, parse_env_vars = False):
    sysroot = sp.check_output(f'$(rustup which --toolchain {TOOLCHAIN} rustc) --print sysroot', shell=True).decode('utf-8').strip()
    p = sp.run(cmd, shell=True, stderr=sp.PIPE, stdout=sp.DEVNULL)
    prefix = 'Running `'

    lines = p.stderr.decode('utf-8').splitlines()
    env_vars = [
        ['SYSROOT', sysroot],
        ['DYLD_LIBRARY_PATH', sysroot + '/lib']
    ]

    if parse_env_vars:
        env_vars += json.loads(lines[0])

    for line in lines[1:]:
        if driver_pattern in line:
            driver_cmd = line.strip()[len(prefix):-1]

    env_str = ' '.join([f'{k}={v}' for [k, v] in env_vars])

    return env_str, driver_cmd
