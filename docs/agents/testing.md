# Testing

## Mutation checks on this checkout

The maintainer's checkout lives on an HFS+ volume, which records file modification times to the second. Cargo decides what to rebuild from those times, so a file changed and restored within the same second is not rebuilt: a test run after restoring can use the build that still holds the mutation, and one run after mutating can use the build without it. Run `touch` on the file after each mutation and after each restore, and leave a second between restoring and the baseline run. Other volumes and CI are not affected.
