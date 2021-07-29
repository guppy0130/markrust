# setup environment on windows.
$ENV:RUSTC_BOOTSTRAP = 1
$ENV:RUSTFLAGS = '-Zinstrument-coverage'
$ENV:LLVM_PROFILE_FILE = 'markrust.profraw'

rustup component add llvm-tools-preview
