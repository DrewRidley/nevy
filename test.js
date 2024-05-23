const tls = require('tls');
const crypto = require('crypto');

const options = {
  host: '127.0.0.1',
  port: 4433,
  rejectUnauthorized: false,
};

const socket = tls.connect(options, () => {
  console.log('Connected to server');
  const cert = socket.getPeerCertificate();

  if (cert.raw) {
    const hash = crypto.createHash('sha256').update(cert.raw).digest('hex');
    console.log('SHA-256 hash of the certificate:', hash);
  } else {
    console.log('Failed to get certificate.');
  }
  socket.end();
});

socket.on('error', (error) => {
  console.error('Error:', error);
});