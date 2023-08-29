# nix-home-fs
A simple FUSE filesystem that allows multiple users to use nix by showing symlinks to the user's home directory

# TODO
* Make it daemonize, so it could be added to `/etc/fstab`. For reference: [sshfs](https://github.com/libfuse/sshfs/blob/c91eb9a9a992f1a36c49a8e6f1146e45b5e1c8e7/sshfs.c#L4392) uses [fuse_daemonize](https://github.com/libfuse/libfuse/blob/869a4a6fa550ae054df01f9d50db68871f88ca4f/lib/helper.c#L253)