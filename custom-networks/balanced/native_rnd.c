#include <stdatomic.h>
#include <stdlib.h>

static _Alignas(4) atomic_int cnt = 0;

void test_exit_rnd(void)
{
	if (atomic_fetch_add_explicit(&cnt, 1, memory_order_acquire) == 0) {
		exit(0);
	}
}
