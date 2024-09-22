down-local-mongo:
	podman stop mongodb

build:
	podman build -t guardian .

local-mongo:
	podman run -d --rm --name mongodb \
	-e MONGODB_ROOT_PASSWORD="admin123456789" \
	-e MONGODB_USERNAME="guardian-user" -e MONGODB_PASSWORD="admin123456789" -e MONGODB_DATABASE="guardian" \
	-p 27017:27017 bitnami/mongodb:latest

local-mongo-arm:
	podman run -d --rm --name mongodb \
	-e MONGO_INITDB_ROOT_USERNAME="guardian-user" \
	-e MONGO_INITDB_ROOT_PASSWORD="admin123456789" \
	-e MONGO_INITDB_DATABASE="guardian" \
	-p 27017:27017 mongodb/mongodb-community-server

certs:
	mkdir certs
	@openssl req -x509 -newkey rsa:2048 -nodes -keyout certs/key.pem -out certs/cert.pem -days 365 -subj '/CN=open.accelerator.cafe'

gen_curve:
	@openssl ecparam -name prime256v1 -genkey -noout -out certs/private.ec.key
	@openssl ec -in certs/private.ec.key -pubout -out certs/public-key.pem
