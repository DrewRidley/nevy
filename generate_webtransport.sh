#!/bin/bash

# Define the certificate file
CERT_FILE="localhost+2.pem"

# Check if the certificate file exists
if [ ! -f "$CERT_FILE" ]; then
  echo "Certificate file $CERT_FILE not found!"
  exit 1
fi

# Extract the SHA-256 fingerprint
FINGERPRINT=$(openssl x509 -in "$CERT_FILE" -noout -sha256 -fingerprint | sed 's/SHA256 Fingerprint=//' | tr -d :)

# Convert hex to base64
BASE64_FINGERPRINT=$(echo "$FINGERPRINT" | xxd -r -p | base64)

# Output the JavaScript code
cat <<EOL
(async () => {
    try {
        const url = 'https://127.0.0.1:4433/';
        const options = {
            serverCertificateHashes: [
                {
                    algorithm: 'sha-256',
                    value: '$BASE64_FINGERPRINT' // Use the output from the script
                }
            ]
        };

        const transport = new WebTransport(url, options);

        transport.closed
            .then(() => {
                console.log('WebTransport connection closed gracefully.');
            })
            .catch(error => {
                console.error('WebTransport connection closed with error:', error);
            });

        await transport.ready;
        console.log('WebTransport connection established.');

        // Optionally, set up streams for bidirectional communication
        const stream = await transport.createBidirectionalStream();
        console.log('Bidirectional stream created:', stream);

    } catch (error) {
        console.error('Failed to establish WebTransport connection:', error);
        console.error('Error details:', error.toString());
        console.error('Error stack:', error.stack);
    }
})();
EOL
