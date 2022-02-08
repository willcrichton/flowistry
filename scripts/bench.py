import subprocess as sp
import json
import sys

TOOLCHAIN = 'nightly-2022-01-31'
CMD = ' '.join(sys.argv[1:])

sysroot = sp.check_output(f'$(rustup which --toolchain {TOOLCHAIN} rustc) --print sysroot', shell=True).decode('utf-8').strip()
cmd = f'cargo +{TOOLCHAIN} flowistry --bench {CMD} > /dev/null'
p = sp.run(cmd, shell=True, stderr=sp.PIPE)
prefix = 'Running `'

lines = p.stderr.decode('utf-8').splitlines()
env_vars = json.loads(lines[0])

for line in lines[1:]:
  if 'flowistry-driver' in line:
    driver_cmd = line.strip()[len(prefix):-1]

env = {k: v for [k, v] in env_vars}
env['SYSROOT'] = sysroot
env_str = ' '.join([f'{k}={v}' for [k, v] in env_vars])
print(f"{env_str} xcrun xctrace record --template 'Time Profiler' --launch -- {driver_cmd}")