FROM ubuntu:22.04
LABEL maintainer="Primus Labs"
WORKDIR /zkvm-server
ENV EXECUTION_FLAG=DOCKER
COPY server/ /zkvm-server

RUN apt-get update && \
    apt-get install -y python3 python3-pip && \
    ln -sf python3 /usr/bin/python && \
    apt-get clean

RUN chmod +x /zkvm-server/bin/zktls /zkvm-server/https_server.py
RUN pip3 install -r /zkvm-server/requirements.txt

EXPOSE 38080
CMD ["python", "https_server.py"]