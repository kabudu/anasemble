# Binary installation

Verify the archive digest against `SHA256SUMS`, extract it into a clean
directory, and install the executable into a new owned prefix:

```text
./anasemble install /opt/anasemble-0.1.0-rc.1
```

Anasemble has no generic help command: invoking it without a complete domain
command fails closed and prints the command inventory. See the versioned
[installation guide](https://github.com/kabudu/anasemble/blob/v0.1.0-rc.1/docs/INSTALLATION.md)
for lifecycle and removal instructions.
