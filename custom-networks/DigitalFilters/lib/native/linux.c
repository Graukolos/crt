#include <stdio.h>
#include <stdlib.h>

#define MAX_SRC 16
#define TOTAL 1000000L

static int *data[MAX_SRC];
static long len[MAX_SRC];
static long pos[MAX_SRC];
static FILE *sink_file;
static long consumed = 0;

int native_source_init(int index)
{
	char name[64];
	FILE *f;
	long cap = 1024;
	int v;
	if (index < 0 || index >= MAX_SRC)
		index = 0;
	sprintf(name, "input_%i.in", index);
	f = fopen(name, "r");
	if (!f)
		return 0;
	data[index] = malloc(cap * sizeof(int));
	while (fscanf(f, "%i", &v) == 1) {
		if (len[index] == cap) {
			cap *= 2;
			data[index] = realloc(data[index], cap * sizeof(int));
		}
		data[index][len[index]++] = v;
	}
	fclose(f);
	return (int)len[index];
}

static int next(int index)
{
	if (index < 0 || index >= MAX_SRC)
		index = 0;
	if (!data[index])
		native_source_init(index);
	return data[index][pos[index]++ % len[index]];
}

int native_source_produce(int index)
{
	return next(index);
}

float native_source_produce_sf(int index)
{
	return (float)next(0);
}

void native_sink_init(void)
{
	sink_file = fopen("sink.out", "w");
}

static void count(void)
{
	if (++consumed == TOTAL) {
		fclose(sink_file);
		exit(0);
	}
}

void native_sink_consume(short v)
{
	fprintf(sink_file, "%i\n", v);
	count();
}

void native_sink_consume_sf(float v)
{
	fprintf(sink_file, "%.0f\n", v);
	count();
}
