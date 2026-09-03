#!/usr/bin/env bash
#
# Build the SquashFS v1 and v2 test images.
#
# backhand downloads its test assets from a URL recorded in test-assets.toml.
# No public source supplies v1 or v2 images, so this script makes them from the
# original tools. Run it, then publish the files in ./images and record their
# sha256 in test-assets.toml.
#
# The tools are from 2004 and do not build on a current toolchain. This script
# applies the necessary corrections; each one has a comment that says why.
#
# Usage: ./generate-v1-v2-images.sh [output-dir]

set -euo pipefail

OUT=$(realpath "${1:-./images}")
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

SQ22_URL=https://github.com/Freetz-NG/dl-mirror/releases/download/dl/squashfs2.2-r2.tar.gz
SQ13_URL=https://downloads.sourceforge.net/project/squashfs/squashfs/squashfs1.3r3/squashfs1.3r3.tar.gz
LZMA_URL=https://github.com/Freetz-NG/dl-mirror/releases/download/dl/lzma465.tar.bz2

# Old C, current compiler. -fcommon restores the pre-GCC-10 handling of
# tentative definitions, and sys/sysmacros.h supplies major()/minor().
CF="-O2 -D_FILE_OFFSET_BITS=64 -D_LARGEFILE_SOURCE -std=gnu89 -fcommon"
CF="$CF -include sys/sysmacros.h -Wno-incompatible-pointer-types"
CF="$CF -Wno-error=implicit-function-declaration -Wno-error=int-conversion"
CF="$CF -Wno-error=implicit-int"

mkdir -p "$OUT"
cd "$WORK"

echo "==> Downloading sources"
curl -sL "$SQ22_URL" -o sq22.tar.gz
curl -sL "$SQ13_URL" -o sq13.tar.gz
curl -sL "$LZMA_URL" -o lzma465.tar.bz2

echo "==> Building the tree to pack"
mkdir -p tree/dir_a/nested tree/dir_b
echo "hello from the backhand v1/v2 test image" > tree/a.txt
head -c 300000 /dev/urandom > tree/big.bin
printf 'small\n' > tree/dir_a/small.txt
head -c 5000 /dev/zero > tree/dir_a/nested/zeros.bin
echo "second file, to exercise duplicate detection" > tree/dir_a/nested/second.txt
ln -s ../a.txt tree/dir_b/link_to_a
mkfifo tree/dir_b/a_fifo

echo "==> Building mksquashfs 2.2 (v2.0 and v2.1, gzip)"
mkdir -p sq22 && tar xzf sq22.tar.gz -C sq22
(
  cd sq22/squashfs2.2-r2/squashfs-tools
  for s in mksquashfs read_fs sort; do cc $CF -I. -c -o $s.o $s.c; done
  cc $CF mksquashfs.o read_fs.o sort.o -lz -o mksquashfs
)
MK22="$WORK/sq22/squashfs2.2-r2/squashfs-tools/mksquashfs"

"$MK22" tree "$OUT/squashfs_v2_le.sqfs"   -le      -noappend > /dev/null
"$MK22" tree "$OUT/squashfs_v2_be.sqfs"   -be      -noappend > /dev/null
"$MK22" tree "$OUT/squashfs_v2_0_le.sqfs" -le -2.0 -noappend > /dev/null
"$MK22" tree "$OUT/squashfs_v2_0_be.sqfs" -be -2.0 -noappend > /dev/null

echo "==> Building mksquashfs 1.3 (v1.0, gzip)"
mkdir -p sq13 && tar xzf sq13.tar.gz -C sq13
(
  cd sq13/squashfs1.3r3/squashfs-tools
  python3 - <<'PY'
import re
# 1. A cast is not a valid left operand of an assignment. Old compilers took
#    "p = (T *) inode = get_inode(...)"; split it into two statements.
src = open('mksquashfs.c').read()
src, n = re.subn(
    r'(\t+)inodep = \((\w+) \*\) inode = (get_inode\([^;]*?\));',
    lambda m: f'{m.group(1)}inode = {m.group(3)};\n{m.group(1)}inodep = ({m.group(2)} *) inode;',
    src, flags=re.S)
assert n == 2, n

# 2. The duplicate-detection cursor holds a raw pointer in the read_from_buffer
#    case, so it must be pointer-width. "unsigned int" truncates it on a 64-bit
#    host, and mksquashfs segfaults on the first file large enough to check.
for old, new in [
    ("unsigned char *read_from_buffer(unsigned int *start, unsigned int avail_bytes)",
     "unsigned char *read_from_buffer(uintptr_t *start, unsigned int avail_bytes)"),
    ("inline unsigned char *read_from_file(unsigned int *start, unsigned int avail_bytes)",
     "inline unsigned char *read_from_file(uintptr_t *start, unsigned int avail_bytes)"),
    ("unsigned short get_checksum(unsigned char *(get_next_file_block)(unsigned int *, unsigned int), unsigned int file_start, int l)",
     "unsigned short get_checksum(unsigned char *(get_next_file_block)(uintptr_t *, unsigned int), uintptr_t file_start, int l)"),
    ("unsigned int bytes = 0, position = file_start;",
     "unsigned int bytes = 0;\n\tuintptr_t position = file_start;"),
    ("int duplicate(unsigned char *(get_next_file_block)(unsigned int *, unsigned int), unsigned int file_start, int bytes, unsigned short **block_list, int *start, int blocks)",
     "int duplicate(unsigned char *(get_next_file_block)(uintptr_t *, unsigned int), uintptr_t file_start, int bytes, unsigned short **block_list, int *start, int blocks)"),
    ("unsigned int position = file_start;", "uintptr_t position = file_start;"),
    ("duplicate(read_from_buffer, (unsigned int) c_buffer,",
     "duplicate(read_from_buffer, (uintptr_t) c_buffer,"),
    ("#include <stdio.h>", "#include <stdio.h>\n#include <stdint.h>"),
]:
    assert old in src, old[:60]
    src = src.replace(old, new, 1)
open('mksquashfs.c','w').write(src)

# 3. On-disk times are 32 bits. time_t is 64 bits here, which pushes every
#    field after it out of place: in the superblock it corrupts root_inode, and
#    in the reg inode it corrupts start_block, file_size and the block list.
hdr = open('squashfs_fs.h').read()
old = "\ttime_t\t\t\tmkfs_time /* time of filesystem creation */;"
assert old in hdr
hdr = hdr.replace(old, "\tunsigned int\t\tmkfs_time /* time of filesystem creation */;", 1)
assert hdr.count("\ttime_t\t\t\tmtime;") == 2
hdr = hdr.replace("\ttime_t\t\t\tmtime;", "\tunsigned int\t\tmtime;")
open('squashfs_fs.h','w').write(hdr)
PY
  for s in mksquashfs read_fs; do cc $CF -I. -c -o $s.o $s.c; done
  cc $CF mksquashfs.o read_fs.o -lz -o mksquashfs
)
MK13="$WORK/sq13/squashfs1.3r3/squashfs-tools/mksquashfs"

# mksquashfs 1.3 appends to an existing file, so the target must not exist.
rm -f "$OUT/squashfs_v1_le.sqfs"
"$MK13" tree "$OUT/squashfs_v1_le.sqfs" -noappend > /dev/null

echo "==> Building mksquashfs-lzma (v2 minor 76, the AVM/Freetz LZMA variant)"
mkdir -p lzma && tar xjf lzma465.tar.bz2 -C lzma
(
  cd lzma/C
  cc -O2 -D_7ZIP_ST -c LzmaLib.c LzmaEnc.c LzmaDec.c LzFind.c Alloc.c
  ar rcs liblzma_sdk.a LzmaLib.o LzmaEnc.o LzmaDec.o LzFind.o Alloc.o
)
mkdir -p sq22lzma && tar xzf sq22.tar.gz -C sq22lzma
(
  cd sq22lzma/squashfs2.2-r2/squashfs-tools
  # Freetz's 300-lzma.patch expects a LZMA_ZLibCompat.h that ships with its own
  # LZMA package. This is the same shim over the stock SDK. The block layout is
  # the "LZMA alone" one that sasquatch's lzma_standard_uncompress() reads:
  # 5 property bytes, the size as 8 little-endian bytes, then the stream.
  cat > LZMA_ZLibCompat.h <<'EOF'
#ifndef LZMA_ZLIB_COMPAT_H
#define LZMA_ZLIB_COMPAT_H

#include <string.h>
#include "LzmaLib.h"

#define LZMA_ZLIB_COMPAT(name) lzma_zlib_##name

#define LZMA_ZC_PROPS_SIZE 5
#define LZMA_ZC_HEADER_SIZE (LZMA_ZC_PROPS_SIZE + 8)

static int lzma_zlib_compress2(unsigned char *dest, unsigned long *destLen,
		const unsigned char *src, unsigned long srcLen, int level)
{
	size_t props_size = LZMA_ZC_PROPS_SIZE;
	size_t outlen = *destLen - LZMA_ZC_HEADER_SIZE;
	int i, res;

	(void) level;
	res = LzmaCompress(dest + LZMA_ZC_HEADER_SIZE, &outlen, src, srcLen,
			dest, &props_size, 5, 1 << 20, 3, 0, 2, 32, 1);
	if(res != SZ_OK)
		return -1;

	for(i = 0; i < 8; i++)
		dest[LZMA_ZC_PROPS_SIZE + i] = (unsigned char)
			(i < 4 ? (srcLen >> (i * 8)) & 0xff : 0);

	*destLen = outlen + LZMA_ZC_HEADER_SIZE;
	return 0;
}

static int lzma_zlib_uncompress(unsigned char *dest, unsigned long *destLen,
		const unsigned char *src, unsigned long srcLen)
{
	size_t outlen = *destLen;
	size_t inlen = srcLen - LZMA_ZC_HEADER_SIZE;
	int res;

	res = LzmaUncompress(dest, &outlen, src + LZMA_ZC_HEADER_SIZE, &inlen,
			src, LZMA_ZC_PROPS_SIZE);
	if(res != SZ_OK)
		return -1;

	*destLen = outlen;
	return 0;
}

#endif
EOF
  python3 - <<'PY'
# The three semantic changes from Freetz's 300-lzma.patch, applied directly
# because that patch is cut against Freetz's copy of the 2.2 tree.
hdr = open('squashfs_fs.h').read()
old = "#define SQUASHFS_MINOR\t\t\t1"
assert old in hdr
open('squashfs_fs.h','w').write(
    hdr.replace(old, old + "\n#define SQUASHFS_MINOR_LZMA\t\t76", 1))

shim = ('#if defined(USE_LZMA_COMPRESSION)\n'
        '#include "LZMA_ZLibCompat.h"\n'
        '#define compress2  LZMA_ZLIB_COMPAT(compress2)\n'
        '#define uncompress LZMA_ZLIB_COMPAT(uncompress)\n'
        '#endif\n\n')
for f in ('mksquashfs.c', 'read_fs.c'):
    s = open(f).read()
    assert '#include <squashfs_fs.h>' in s, f
    open(f,'w').write(s.replace('#include <squashfs_fs.h>',
                                shim + '#include <squashfs_fs.h>', 1))

s = open('mksquashfs.c').read()
old = "int filesystem_minor_version = SQUASHFS_MINOR;"
assert old in s
open('mksquashfs.c','w').write(s.replace(old,
    "#if defined(USE_LZMA_COMPRESSION)\n"
    "int filesystem_minor_version = SQUASHFS_MINOR_LZMA;\n"
    "#else\n" + old + "\n#endif", 1))
PY
  SDK="$WORK/lzma/C"
  for s in mksquashfs read_fs sort; do
    cc $CF -I. -I"$SDK" -DUSE_LZMA_COMPRESSION -D_7ZIP_ST -c -o $s.o $s.c
  done
  cc $CF mksquashfs.o read_fs.o sort.o "$SDK/liblzma_sdk.a" -o mksquashfs-lzma
)
MKL="$WORK/sq22lzma/squashfs2.2-r2/squashfs-tools/mksquashfs-lzma"

"$MKL" tree "$OUT/squashfs_v2_lzma_le.sqfs" -le -noappend > /dev/null
"$MKL" tree "$OUT/squashfs_v2_lzma_be.sqfs" -be -noappend > /dev/null

# mksquashfs creates its output 0700, which a web server cannot read once the
# images are published.
chmod 644 "$OUT"/squashfs_v[12]*.sqfs

echo
echo "==> Images written to $OUT"
for f in "$OUT"/squashfs_v[12]*.sqfs; do
  printf '%-40s %s  %s\n' "$(basename "$f")" \
    "$(head -c 4 "$f" | xxd -p)" "$(sha256sum "$f" | cut -c1-16)"
done
echo
echo "Verify the gzip images with a current unsquashfs, which still reads v1"
echo "and v2 (but not the minor-76 LZMA variant):"
echo "  unsquashfs -d /tmp/out $OUT/squashfs_v2_le.sqfs"
