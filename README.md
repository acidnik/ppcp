ppcp
====

Command-line tool for copying files and directories with progress bar

WARNING
=======

This is an early stage software. Do not use it for anything serious. Please send feedback via github issues

USAGE
=====
```
# copy file to dir
ppcp <path/to/file> <path/to/dest/dir>

# copy file to file
ppcp <path/to/file> <path/to/dest/file>

# copy dir to dir. directory /path/to/dest/dir will be created
ppcp <path/to/dir> <path/to/dest>

# copy multiple files/dirs
ppcp <path/to/file1> <path/to/dir2> <path/to/dest>
```

Error handling
--------------
Currently, ppcp will panic on any error. TODO is to add a dialog asking abort/skip/skip all/retry/overwrite and command-line option for default actions

Alternatives
------------
```
rsync -P
```
https://code.lm7.fr/mcy/gcp
