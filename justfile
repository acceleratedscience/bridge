down:
	docker compose down

certs:
	mkdir certs
	@openssl req -x509 -newkey rsa:2048 -nodes -keyout certs/key.pem -out certs/cert.pem -days 365 -subj '/CN=localhost'

gen_curve:
	@openssl ecparam -name prime256v1 -genkey -noout -out certs/private.ec.key
	@openssl ec -in certs/private.ec.key -pubout -out certs/public-key.pem
