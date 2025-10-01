# Unimplemented mksquashfs arguments:

## Filesystem compression options:

- -noI
- -noId
- -noD
- -noF
- -noX
- -no-compression

## Filesystem build options:

- -tar
- -cpiostyle
- -cpiostyle0
- -reproducible
- -all-time
- -root-time
- -no-exports
- -exports
- -no-sparse
- -tailends
- -no-fragments
- -no-hardlinks
- -keep-as-directory

## Filesystem filter options:

- -sort
- -ef
- -wildcards
- -regex
- -one-file-system
- -one-file-system-x

## Filesystem xattrs options:

- -xattrs-exclude
- -xattrs-include
- -xattrs-add

## Runtime options:

- -exit-on-error
- -info
- -percentage
- -throttle
- -limit
- -processors
- -mem
- -mem-percent

## Filesystem append options:

- -root-becomes

## Expert:

- -offset

## Tar file only options:

- -default-mode
- -default-uid
- -default-gid
- -ignore-zeros

## Compression options:

- gzip:
  - -Xstrategy
- lzo:
  - -Xalgorithm
  - -Xcompression-level
- lz4:
  - Xhc

## Deliberately unimplemented:

- -tarstyle
- -reproducible
- -not-reproducible
- -no-tailends
- -pseudo-override
- -p
- -pf
- -no-xattrs
- -xattrs
- -nopad
- -no-progress
- -progress
- -mem-default
- -no-recovery
- -recovery-path
- -recover
- -action
- -log-action
- -true-action
- -false-action
- -action-file
- -log-action-file
- -true-action-file
- -false-action-file

## Deliberately changed:

- -nopad => --padding
- -root-mode => --root-dir-mode
- -root-uid => --root-dir-uid
- -root-gid => --root-dir-gid
- -noappend => -append
- -Xwindow-size => --Xgzip-window-size
- -Xbcj => --Xxz-bcj
- -Xdict-size => --Xxz-dict-size without the ability to specify a percentage of block size
- No lzma compression support
