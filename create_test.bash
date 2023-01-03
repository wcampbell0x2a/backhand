#!/bin/bash
set -ex
rm -rf testing
for (( a=0; a<1; a++ ))
do
    empty=$(xxd -l 1 -c 1 -p < /dev/random)
    mkdir -p testing/$empty
    empty=$(xxd -l 1 -c 1 -p < /dev/random)
    mkdir -p testing/$empty

    parent=$(xxd -l 1 -c 1 -p < /dev/random)
    for (( b=0; b<5; b++ ))
    do
        child1=$(xxd -l 1 -c 1 -p < /dev/random)
        for (( c=0; c<1; c++ ))
        do
            child2=$(xxd -l 1 -c 1 -p < /dev/random)
            mkdir -p testing/$parent/$child1/$child2
            file=$(xxd -l 1 -c 1 -p < /dev/random)
            dd if=/dev/random of=testing/$parent/$child1/$child2/$file bs=5 count=1
            file=$(xxd -l 1 -c 1 -p < /dev/random)
            dd if=/dev/random of=testing/$parent/$child1/$child2/$file bs=5 count=1
            file=$(xxd -l 16 -c 16 -p < /dev/random)
            dd if=/dev/random of=testing/$parent/$child1/$child2/$file bs=5 count=1
            file=$(xxd -l 16 -c 16 -p < /dev/random)
            dd if=/dev/random of=testing/$parent/$child1/$child2/$file bs=500 count=1
            file=$(xxd -l 16 -c 16 -p < /dev/random)
            dd if=/dev/random of=testing/$parent/$child1/$child2/$file bs=1000 count=1
            file=$(xxd -l 16 -c 16 -p < /dev/random)
            dd if=/dev/random of=testing/$parent/$child1/$child2/file2 bs=131100 count=1
        done
    done
done

rm -rf out.squashfs
mksquashfs testing out.squashfs -comp xz
