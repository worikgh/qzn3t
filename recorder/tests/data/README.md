# Test Data Directory

This is a directory where tests write data.

Use this rather than `tempfile::tempdir()` so that in case of failed
tests the data that was written can be conveniently checked
