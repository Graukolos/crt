#include <stdlib.h>

static volatile int cnt = 0;

void test_exit(void)
{
	if (__atomic_add_fetch(&cnt, 1, __ATOMIC_ACQUIRE) == 16) {
		exit(0);
	}
}
