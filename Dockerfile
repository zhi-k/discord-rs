FROM rust as builder

RUN apt-get update && apt-get -y install ca-certificates cmake musl-tools libssl-dev openssl pkg-config

# ------------------------------- Build OpenSSL for the `musl` build target
RUN \
  ln -s /usr/include/x86_64-linux-gnu/asm /usr/include/x86_64-linux-musl/asm && \
  ln -s /usr/include/asm-generic /usr/include/x86_64-linux-musl/asm-generic && \
  ln -s /usr/include/linux /usr/include/x86_64-linux-musl/linux

WORKDIR /musl

RUN wget https://github.com/openssl/openssl/archive/OpenSSL_1_1_1f.tar.gz
RUN tar zxvf OpenSSL_1_1_1f.tar.gz 
WORKDIR /musl/openssl-OpenSSL_1_1_1f/

RUN CC="musl-gcc -fPIE -pie" ./Configure no-shared no-async --prefix=/musl --openssldir=/musl/ssl linux-x86_64
RUN make depend
RUN make -j$(nproc)
RUN make install

WORKDIR /usr/app

COPY . .

RUN rustup target add x86_64-unknown-linux-musl

ENV PKG_CONFIG_ALLOW_CROSS=1 OPENSSL_DIR=/musl

RUN cargo build -v --target x86_64-unknown-linux-musl --release

FROM scratch

COPY --from=builder /usr/app/target/x86_64-unknown-linux-musl/release/discord-rs .

CMD ["./discord-rs"]