CC = gcc
CFLAGS = -std=c11 -pthread -D_GNU_SOURCE
TARGETS = bin/test-for-futex bin/test-for-pthread-mutex bin/test-for-robust-futex
SRC = test-for-futex.c test-for-pthread-mutex.c test-for-robust-futex.c

all: $(TARGETS)

bin/test-for-futex: test-for-futex.c
	mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -o $@ $<

bin/test-for-pthread-mutex: test-for-pthread-mutex.c
	mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -o $@ $<

bin/test-for-robust-futex: test-for-robust-futex.c
	mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -o $@ $<

clean:
	rm -rf bin

.PHONY: all clean