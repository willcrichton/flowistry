import subprocess as sp
import json
import toml
import pathlib
import platform

PLATFORM_VARS = {
    'Darwin': {
        'library_path': 'DYLD_LIBRARY_PATH',
        'tracing_command': "xcrun xctrace record --template 'Time Profiler' --launch --"
    },
    'Linux': {
        'library_path': 'LD_LIBRARY_PATH',
        'tracing_command': 'perf record'
    }
}[platform.system()]

def get_toolchain():
    curr_dir = pathlib.Path(__file__).parent.resolve()
    project_dir = curr_dir.parent.parent
    toml_data = toml.load(project_dir / 'rust-toolchain.toml')

    return toml_data['toolchain']['channel']

TOOLCHAIN = get_toolchain()

def get_cargo_cmd(cmd: str, driver_pattern: str, parse_env_vars = False):
    sysroot = sp.check_output(f'$(rustup which --toolchain {TOOLCHAIN} rustc) --print sysroot', shell=True).decode('utf-8').strip()
    p = sp.run(cmd, shell=True, stderr=sp.PIPE, stdout=sp.DEVNULL)
    prefix = 'Running `'

    lines = p.stderr.decode('utf-8').splitlines()
    env_vars = [
        ['SYSROOT', sysroot],
        [PLATFORM_VARS['library_path'], sysroot + '/lib']
    ]

    if parse_env_vars:
        env_vars += json.loads(lines[0])

    for line in lines[1:]:
        if driver_pattern in line:
            driver_cmd = line.strip()[len(prefix):-1]

    env_str = ' '.join([f'{k}={v}' for [k, v] in env_vars])

    return env_str, driver_cmd
