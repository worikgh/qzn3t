# Compose

Preconditions
---

* The repository for [Qznrt](https://github.com/worikgh/qzn3t.git) is a sibling directory
* Mplayer is installed at `/usr/bin/mplayer`
* Sox is at `/usr/bin/sox`

Usage
---

`compose [-p <prefix>] [-b <backing track>] [-d <directory>]`

Control with the keyboard

Starting a new recording
---

Press key `n` and then `<enter>`

Any key press stops recording

Every jack input will be recorded to a separate track, and the tracks will be placed in a directory

It will be recorded in raw format, and wav.

TODO:  Describe how directories organised

Dubbing
---

Any key but `q` or `n`

One of the tracks will be chosen and played pack

The recording will happen as normal

One of the tracks recorded above will be chosen
