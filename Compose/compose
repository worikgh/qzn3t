#!/usr/bin/perl -w
use strict;
## Debug with: o inhibit_exit=0

## Compose
(my $DIR = $0) =~ s/[^\/]+$//;
$DIR =~ s/\/$//; # Get rid of trailing slash
$DIR = '.' if !length($DIR);
warn "DBG compose: \$DIR: $DIR\n";

## Directory to write all files to
my $DATADIR = "$DIR/compositions";
-d $DATADIR or mkdir($DATADIR) or die "$!: Cannot mkdir $DATADIR ";

## Hold the PID of any children so they can be killed on surprise
## exit
my @PIDS = ();
$SIG{INT} = sub {
    print "\nShutting down gracefully...\n";
    kill 'TERM', @PIDS;
    exit;
};
## Locate Qzn3t root
my $QZN3T = "$DIR/..";
-d $QZN3T or die "$!: $QZN3T is not a directory";

## Programme to measure max amplitude of raw audio to reject silent files
my $AMPLITUDE = "$QZN3T/peak_volume/target/release/peak_volume";
-x $AMPLITUDE or die "$!:  $AMPLITUDE";

## SoX audio editing software
my $SOX = "/usr/bin/sox";
-x $SOX or die "$!: $SOX is not executable";

## Facilitate composing live loops.  Recording via Jack and overdubbing

## Records everything connected to Jack with `qzn3t/jack_rec`
## Converts the recordings to WAV files

## Usage:
sub usage {
    print STDERR "120Proof/bin/Compose [-p <prefix>] [-b <backing track>] [-d <directory>]\n";
}

# o inhibit_exit=0
## `$file_pfx` is prefixed to all track file names.  Over ride with `-p`
my $file_pfx = &std_pfx();

# `$directory` to write to
my $directory = &std_dir();

# When overdubbing this is the track played
my $backing_track = undef;

# When overdubbing this is the recorded track
my $dub_track = undef;

## The `-i` argument sets which Jackd pipes to record.  E.g: `-i
## yoshimi:left -i PureData::out1` .  If empty the argument is not
## used and `jack_rec` makes up its own mind
my @inputs = ();

## Process command line
my $ARGC = @ARGV;
for(my $i = 0; $i < ($ARGC - 1); $i+=2){

    ## The input pipe
    if($ARGV[$i] eq "-i"){
	my $pipe = $ARGV[$i+1];
	if(! grep{$_ eq $pipe} map{chomp; $_} `jack_lsp`) {
	    die "Error compose: -i $pipe: Not a valid Jackd pipe";
	}
	my $pipe_type = `jack_lsp -t|grep '^$pipe\$' -A 1|tail -n 1`;
	if($pipe_type !~ / audio$/){
	    die "Error compose: The input pipe: $pipe has invalid type: $pipe_type";
	}
	push(@inputs, $pipe);
	next;
    }

    ## The prefix to use (defaults to YYYYMMDDhhmmss)
    if($ARGV[$i] eq "-p"){
	$file_pfx = $ARGV[$i+1];
	next;
    }

    ## If a backing track passed recording the first track will not
    ## happen and it goes go straight to dubbing
    if($ARGV[$i] eq "-b" ){
	$backing_track = $ARGV[$i+1];
	next;
    }

    ## A directory can be passed.  This is used in audio/
    if($ARGV[$i] eq "-d"){
	$directory = $ARGV[$i+1];
	next;
    }
    &usage;
    exit;
}

{
    ## `jack_rec`  is a programme that records Jackd outputs as raw audio
    my $JACKREC = "$QZN3T/jack_rec/target/release/jack_rec";
    -e $JACKREC or die "$!:  $JACKREC is not executable";
    sub jack_rec {
	my $pfx = shift or die "Pass a prefix";
	my $inputs = join(' ', map{"-i \"$_\""} @inputs);
	my $cmd = "$JACKREC -p \"$pfx\" $inputs";
	return $cmd;
    }
}
## Overdubs go in the same directory as the recordings, with the same
## name, with this prefix
my $dub = 1; ## Name dubs with this

# Set to the raw recording file name, and dub name, not extension, of
# a recording
my $fn_rec = undef;
my $fn_dub = undef;

## ?????
my $fn_dir = undef;

# The copy of `$file_pfx` used in recording
my $pfx = $file_pfx;

## Used to replay the recording while overdubbing
my $PLAY = "/usr/bin/mplayer";
-x $PLAY or die "$!: $PLAY";

## State machine:
## recording => recording a track
my $recording = 'recording';
## dubbing => playing back and recording
my $dubbing = 'dubbing';
## Reviewing a recording
my $rec_review = "recording_review";
# Accept, or not, a recording
my $rec_accept = 'rec_accept';
## Reviewing the result of dubbing
my $dub_review = "dub_review";
## Accept or not the dubing
my $dub_accept = 'dub_accept';

# Initial state. If `-b <backing track>` on command line a backing
# track is ready for dubbing
my $state  = defined $backing_track  ? $dubbing : $recording;

## Mesages to display to the user for each state
my %state_msg = (
    $recording => "Press \n<enter> to start recording",
    $dubbing => "Press \n<enter> to start overdubbing",
    $rec_review => "Press \n<enter> to review recording \nr <enter> to record again \nd <enter> to overdub",
    $dub_review => "Press \n<enter> to review dub \nd <enter> to dub again \nr <enter> to record again",
    $dub_accept => "Press \nd <enter> to dub again \nr <enter> to record again\ng <enter> Review again",
    );


## If the programme is initialised with a backing track go straight to
## dubbing mode
if(defined $backing_track){
    if(! -r $backing_track){
	die "Unreadable backing track: $backing_track ";
    }else{
	$state = $dubbing;

	## If the backing track is one this programme has made that can be
	## used to help in the file name
	my $id = "";
	if($backing_track =~ /\d{14}(\S+).wav$/){
	    $id = '_'.$1.'_';
	}
	## Other wise if the file name is a reasonable name use it as id
	elsif($backing_track =~ /([a-zA-Z_\-\d\:\.]+)\.wav$/){
	    $id = '_'.$1.'_';
	}
	$pfx = $file_pfx.$id;

	## `$fn_rec` is the track to be played under overdubbing
	$backing_track =~ /([^\/]+)\.wav$/ or $backing_track =~ /([^\/]+)\.flac$/ or die "Backing track is not WAV nor FLAC\n$backing_track\n";
	$fn_rec = $1;
    }
}

# The directory to write data to.
my $audio_dir = "$DATADIR/audio/$directory";
`mkdir -p $audio_dir`;
-d $audio_dir or die "$!: Audio directory: $audio_dir does not exist";

## For all states append this message to the display

## Recorded files go in a directory named `$fn`
my $fn = 1;

warn "DBG compose: fn_rec: $fn_rec\n";
while(1){

    print "State: $state\n$state_msg{$state}\n";
    my $inp = <STDIN>;
    chomp $inp;
    $inp eq 'q' and last;

    if($state eq $dubbing || $state eq $recording){
	$fn_dir = "$audio_dir/$fn";
	-d $fn_dir or mkdir($fn_dir) or die "$!: mkdir $fn_dir";

	# `$fn_rec` is the stem of the file name.  The name assigned
	# by jack_rec will be appended to it
	$fn_rec = "$fn_dir/$file_pfx";
    }

    if($state eq $recording){

	$backing_track = undef;

	print "Press <enter> to stop recording\n";
	## This blocks untill a line is entered from keyboard (enter
	## key is pressed) and returns a JSON object
	my $cmd = &jack_rec($fn_rec);
	my $result = `$cmd`;

	print "Processing...\n";
	my @out_file_stats = &process_jackrec($result);
	@out_file_stats or die "Error compose: No file data returned after processing recording";
	$backing_track = &get_peakiest_file(@out_file_stats);

	if(!defined($backing_track)){
	    print "No matching file has audio in it. ".
		"Cannot make a backing track for dubbing";
	    $state = $recording;
	}else{

	    $state = $rec_review;

	    my $_bt = $backing_track;
	    $_bt =~ s/^$audio_dir/$directory/ or die "$_bt";
	    print "$_bt\n";
	}
	next;
    }elsif($state eq $rec_review){

	if(lc($inp) eq 'r'){
	    $state = $recording;
	}elsif(lc($inp) eq 'd'){
	    ## TODO  Where is backing track set?
	    $state = $dubbing;
	}else{
	    `$PLAY -ao jack "$backing_track"`;
	}
    }elsif($state eq $dubbing){
	$fn_dub = $fn_rec."-$dub";
	$dub++; # Finished with this for now, get ready for next dub
	print "Press <enter> to stop overdubbing\n";
	my $pid = run_daemon("$PLAY -ao jack $backing_track") or die;
	push(@PIDS, $pid);

	## This blocks and returns a JSON object
	my $cmd = &jack_rec($fn_dub);
	my $result = `$cmd`;
	`pkill -f $PLAY`;
	warn "Processing\nResult: $result\n";
	my @out_file_stats = &process_jackrec($result);
	@out_file_stats or die "Error compose: No file data returned after processing recording";
	$dub_track = &get_peakiest_file(@out_file_stats);

	$state = $dub_review;
    }elsif ($state eq $dub_review){
	if(lc($inp) eq 'd'){
	    $state = $dubbing;
	}elsif(lc($inp) eq 'r'){
	    $state = $recording;
	}else{
	    ## TODO  Where is backing track set?
	    warn "Reviewing Dub: \$backing_track: $backing_track ";
	    warn "Reviewing Dub: \$dub_track: $dub_track ";
	    my $p1 = run_daemon("$PLAY -ao jack $backing_track") or die;
	    my $p2 = run_daemon("$PLAY -ao jack $dub_track") or die;
	    push(@PIDS, ($p1, $p2));
	    $state = $dub_accept;
	}
    }elsif($state eq $dub_accept){
	`pkill -f $PLAY`;
	@PIDS = ();
	if($inp eq 'r'){
	    $state = $recording;
	}elsif($inp eq 'd'){
	    $state = $dubbing;
	}elsif($inp eq 'g'){
	    $state = $dub_review;
	}
    }
}

sub process_jackrec {

    ## Process the raw audio from a recording.
    ## Return a hash of the wav files generated and their stats
    my $start  = time();
    my %result = ();
    my $result = shift or die "Pass jackrec_qzt output";
    $result =~ /output_files\": \[\s+(.+)\s+\]\s+}$/s;
    my $fns = $1;
    $fns =~ s/\s*\n\s*//g;
    my @fns = map{s/\"(.+)\"$/$1/; $_} split(/,/, $fns);


    foreach my $fn (@fns) {
	#        open(my $fh, $fn) or die "$!: $fn";
	my $amp = `$AMPLITUDE "$fn"`;
	# Only process files with audio
	if($amp > 0.01){
	    my $fn2 = $fn;
	    $fn2 =~ s/raw$/wav/;
	    # SOX command: sox -t raw -b 32 -e float -c 1 -r 48k "-1_yoshimi-Song1:left.raw" -e signed-integer -b 16  "-1_yoshimi-Song1:left.wav"
	    my $sox_cmd = "$SOX -q -t raw -b 32 -e float -c 1 -r 48k \"$fn\" -e signed-integer -b 16  \"$fn2\"";

	    `$sox_cmd `;
	    my $peaks = peaks($fn2);
	    $result{$fn2} = $peaks;
	    warn "DBG compose: fn2: $fn2\n";
	    $fn2 =~ s/.*\/?([^\/]+).*$/$1/;
	    warn sprintf("Peaks %0.4f Output $fn2\n", $peaks);
	}
    }

    foreach my $fn (@fns){
	unlink $fn or die "$!: unlink $fn";
    }
    print("Processing files took: ".(time() - $start)." seconds\n");

    # Return the files
    my @ret = ();
    my @possible_backing =  keys %result;
    my $avg = -9999999;
    foreach my $p (@possible_backing){
	my $_a = $result{$p};
	if($_a != 0 and $_a > $avg){
	    $avg = $_a;
	    push(@ret, [$_a, $p]);

	}
    }
    return @ret;

}
# ffmpeg -i fooooo.wav -af astats=metadata=1:reset=1,ametadata=print:key=lavfi.astats.Overall.RMS_level:file=log.txt -f null -

sub get_peakiest_file {
    my @out_file_stats = @_;
	my $p_fn = $out_file_stats[0];
	my $p = $p_fn->[0];
	my $fn = $p_fn->[1];
	foreach $p_fn (@out_file_stats){
	    if($p_fn->[0] > $p){
		$fn = $p_fn->[1];
	    }
	}
    return $fn;
}

sub std_pfx {
    return "Compose";
}
sub std_dir {
    my @t = localtime();
    my $wday = ('Sun', 'Mon', 'Tue', "Wed", 'Thu', 'Fri', 'Sat')[$t[6]];
    return sprintf("%04d-%02d-%02dT%02d:%02d:%02d_$wday",
		   $t[5]+1900, $t[4]+1, $t[3], $t[2], $t[1], $t[0]);
}


## Calculate the maximum, the average, and the minimum volume
sub peaks {
    my $fn = shift or die;
    -r $fn or die "$!: Cannot read: '$fn' ";
    my $result = `ffmpeg -loglevel quiet -i $fn -af astats=metadata=1:reset=1,ametadata=print:key=lavfi.astats.Overall.RMS_level:file=- -f null - `;
    my @result = grep {/lavfi.astats.Overall.RMS_level=/} map{chomp; $_}split(/\n/, $result);
    my @data = map{/lavfi.astats.Overall.RMS_level=(\S+)/; $1} grep {$_ ne "lavfi.astats.Overall.RMS_level=-inf"} @result;
    my $positive_infinity = 99999999999;
    my $negative_infinity = -1 * $positive_infinity;

    my ($avg, $sum, $max, $min) = (0, 0, $negative_infinity, $positive_infinity);
    my $denom = scalar(@data);

    if ($denom ){
	foreach my $datum (@data){
	    $sum += $datum;
	    $datum > $max and $max = $datum;
	    $datum < $min and $min = $datum;
	    $avg = $sum / $denom;
	}
    }else{
	$max = 0;
	$min = 0;
	$avg = 0;
    }
    if(wantarray()){
	return ($avg, $min, $max);
    }else{
	return $avg;
    }
}


## Run a programme, either as a daemon (this function retutns straight
## away) or wait on its output
sub run_daemon {
    my $cmd = shift or die "Must pass a command to run";
    my $wait = shift or 0;
    ## Prepare command
    $cmd =~ /^(\S+)/ or die "Invalid command: '$cmd'";
    my $_x = $1;
    -x $_x or die "Must pass an executable.  '$_x' is not";

    defined(my $pid = fork())   or die "can't fork: $!";
    $wait and waitpid($pid, 0);
    if (!$pid){

	## Child

	## Create logs for stderr and stdout

	# Get the name of the command by separating it from the path
	my $command_file = $_x;
	$command_file =~ s/^.+\/([^\/]+)$/$1/;

	my $stderr_fn = $DIR."/$command_file.err";
	$stderr_fn =~ /\/\.err$/ and
	    die "No file name for err: \$cmd: '$cmd' ".
	    join("\n", stack_trace());
	open(my $stderr_fh, '>>', $stderr_fn) or die "$!: $stderr_fn";
	open(STDERR, ">&",$stderr_fh) or die "$!:  Cannot redirect STDERR";
	close $stderr_fh;

	my $stdout_fn = $DIR."/$command_file.out";
	$stdout_fn =~ /\/\.out$/ and
	    die "No file name for out: \$cmd: '$cmd' ".
	    join("\n", stack_trace());
	open(my $stdout_fh, '>>', $stdout_fn) or die "$!: $stdout_fn";
	open(STDOUT, ">&",$stdout_fh) or die "$!:  Cannot redirect STDOUT";
	close $stdout_fh;

	## No STDIN
	open STDIN, '<', '/dev/null' or die "Can't read /dev/null: $!";

	# Turn on autoflush
	select(STDOUT);
	$|++;
	select(STDERR);
	$|++;

	exec($cmd);
	exit;
    }
    return $pid;
}
