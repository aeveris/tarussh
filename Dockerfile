FROM rust:alpine

RUN apk add musl-dev

WORKDIR ./src/
COPY . .

RUN cargo install --path .

EXPOSE 22

CMD ["tarussh"]
