import sys
from utils import get_cargo_cmd

BENCH_TEST = ' '.join(sys.argv[1:])
cmd = f'FLOWISTRY_BENCH_TEST={BENCH_TEST} cargo bench -v --bench main'
env_str, driver_cmd = get_cargo_cmd(cmd, 'main', False)

print(f"{env_str} xcrun xctrace record --template 'Time Profiler' --launch -- {driver_cmd}")
