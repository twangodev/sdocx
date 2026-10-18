FROM eclipse-temurin:21-jre-jammy

ADD --chmod=644 --checksum=sha256:dbf930b076c6b9be08d57c449cacefc3bdd6b71ebd59b3066fc0e1f5b14f9423 https://github.com/iBotPeaches/Apktool/releases/download/v3.0.3/apktool_3.0.3.jar /opt/apktool.jar

ENV HOME=/tmp
WORKDIR /output
ENTRYPOINT ["java", "-jar", "/opt/apktool.jar"]
