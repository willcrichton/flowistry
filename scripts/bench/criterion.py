import sys
from utils import get_cargo_cmd, PLATFORM_VARS

BENCH_TEST = ' '.join(sys.argv[1:])
test_arg = f'FLOWISTRY_BENCH_TEST={BENCH_TEST}'
cmd = f'{test_arg} cargo bench -v --bench main'
env_str, driver_cmd = get_cargo_cmd(cmd, 'main', False)

print(f"{env_str} {test_arg} {PLATFORM_VARS['tracing_command']} {driver_cmd}")
