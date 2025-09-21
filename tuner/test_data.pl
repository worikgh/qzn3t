#!/usr/bin/perl -w
use strict;

my $tuner = "target/release/tuner ";
-x $tuner or die "$!: $tuner not executable";

## Samples per test
my $count = 2048000;

# Must attain at least this volume
my $min = 0.001;

# Mean must be closer to zero than this
my $mean = 0.015;

# Time between samples in MS
my $interval = 200;


