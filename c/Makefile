CC = gcc
CFLAGS = -std=c11 -pthread -D_GNU_SOURCE
TARGET = bin/test-for-futex
SRC = test-for-futex.c

all: $(TARGET)

$(TARGET): $(SRC)
	mkdir -p $(dir $(TARGET))
	$(CC) $(CFLAGS) -o $(TARGET) $(SRC)

clean:
	rm -rf bin

.PHONY: all clean