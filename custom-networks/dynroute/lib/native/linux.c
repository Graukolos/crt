#include <stdlib.h>

#define DYN_RUN_SAMPLES 1000000

static const unsigned tag_seq[16] = {
	0, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 0, 1
};

static const unsigned len_seq[16] = {
	7, 7, 3, 5, 7, 1, 4, 7, 2, 7, 6, 3, 7, 5, 1, 7
};

static const int sample_seq[64] = {
	128, 134, 140, 147, 152, 158, 164, 169, 173, 177, 181, 184, 187, 189, 191, 192,
	192, 192, 191, 189, 187, 184, 181, 177, 173, 169, 164, 158, 152, 147, 140, 134,
	128, 122, 116, 109, 104,  98,  92,  87,  83,  79,  75,  72,  69,  67,  65,  64,
	 64,  64,  65,  67,  69,  72,  75,  79,  83,  87,  92,  98, 104, 109, 116, 122
};

static unsigned tag_idx = 0;
static unsigned len_idx = 0;
static unsigned sample_idx = 0;
static unsigned consumed = 0;

unsigned src_tag(void)
{
	return tag_seq[tag_idx++ & 15u];
}

unsigned src_len(void)
{
	return len_seq[len_idx++ & 15u];
}

int src_sample(void)
{
	return sample_seq[sample_idx++ & 63u];
}

void sink_consume(int v)
{
	(void)v;
	++consumed;
	if (consumed == DYN_RUN_SAMPLES) {
		exit(0);
	}
}
