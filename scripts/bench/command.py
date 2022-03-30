import sys
from utils import get_cargo_cmd, TOOLCHAIN

CMD = ' '.join(sys.argv[1:])
cmd = f'cargo +{TOOLCHAIN} flowistry --bench {CMD} > /dev/null'
env_str, driver_cmd = get_cargo_cmd(cmd, 'flowistry-driver', True)

print(f"{env_str} xcrun xctrace record --template 'Time Profiler' --launch -- {driver_cmd}")
