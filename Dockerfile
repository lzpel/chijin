FROM rust:latest
# the build target
ENV OCCT_ROOT=/occt
RUN apt-get update && apt-get install -y git gcc g++ cmake
RUN git clone https://github.com/lzpel/cadrum && sh -c "cd cadrum && cargo test" && rm -rf cadrum