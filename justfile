run:
	cargo run

runr:
	cargo run --release

samply:
    cargo build
    samply record target/debug/space_finder

samplyrd:
    cargo build --profile rdebug
    samply record target/rdebug/space_finder

fix:
    cargo +nightly fmt
    cargo clippy --fix --allow-dirty --allow-staged --all-targets --all-features
    cargo +nightly fmt
    cargo fmt

binaries:
    rm binaries -r || true
    mkdir binaries
    cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.28
    cp target/x86_64-unknown-linux-gnu/release/space_finder binaries/linux_space_finder

    cargo build --release --target x86_64-pc-windows-gnu
    cp target/x86_64-pc-windows-gnu/release/space_finder.exe binaries/windows_space_finder.exe