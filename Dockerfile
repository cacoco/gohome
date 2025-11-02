FROM arm64v8/alpine:latest
ARG EXECUTABLE="gohome"

RUN apk update && apk add --no-cache ca-certificates bash
RUN apk add --update coreutils

COPY static/*.*         /usr/src/assets/
COPY templates/*.hbs    /usr/src/templates/
COPY entrypoint.sh      /usr/src/entrypoint.sh
COPY ${EXECUTABLE}      /usr/src/gohome

ENTRYPOINT [ "/usr/src/entrypoint.sh" ]
