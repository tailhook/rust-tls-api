/// DER-encoded X.509 certificate.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Cert(Vec<u8>);

impl Cert {
    fn looks_like_der(bytes: &[u8]) -> bool {
        // Quick check for certificate validity:
        // https://tools.ietf.org/html/rfc2459#section-4.1
        // ```
        //  Certificate  ::=  SEQUENCE  {
        //       tbsCertificate       TBSCertificate,
        //       signatureAlgorithm   AlgorithmIdentifier,
        //       signatureValue       BIT STRING  }
        // ```
        // and `SEQUENCE` tag is 0x30
        bytes.starts_with(b"\x30")
    }

    /// Construct from DER-encoded.
    pub fn from_der(cert_der: impl Into<Vec<u8>>) -> crate::Result<Cert> {
        let cert_der = cert_der.into();
        if !Self::looks_like_der(&cert_der) {
            return Err(crate::Error::new_other("not a DER-encoded certificate"));
        }
        Ok(Cert(cert_der))
    }

    /// Construct from PEM-DER-encoded.
    pub fn from_pem(cert_der_pem: impl AsRef<[u8]>) -> crate::Result<Cert> {
        let pem = pem::parse_many(cert_der_pem.as_ref());
        let count = pem.len();
        let mut certs: Vec<Cert> = pem
            .into_iter()
            .flat_map(|p| match p.tag == "CERTIFICATE" {
                true => Some(Self::from_der(p.contents)),
                false => None,
            })
            .collect::<Result<_, _>>()?;
        if certs.len() == 1 {
            return Ok(certs.swap_remove(0));
        } else if certs.len() > 1 {
            return Err(crate::Error::new_other("PEM file contains {} certificates"));
        } else if count != 0 {
            return Err(crate::Error::new_other(
                "PEM file contains {} entries, but no certificates",
            ));
        } else if Self::looks_like_der(cert_der_pem.as_ref()) {
            return Err(crate::Error::new_other("PEM file looks like a DER file"));
        } else {
            return Err(crate::Error::new_other(
                "no certificates found in a PEM file",
            ));
        }
    }

    /// Get certificate as DER.
    pub fn get_der(&self) -> &[u8] {
        &self.0
    }

    /// Convert a certificate to PEM format.
    pub fn to_pem(&self) -> String {
        pem::encode(&pem::Pem {
            tag: "CERTIFICATE".to_owned(),
            contents: self.0.clone(),
        })
    }
}

/// DER-encoded.
#[derive(Debug, PartialEq, Clone)]
pub struct PrivateKey(Vec<u8>);

impl PrivateKey {
    fn looks_like_der(bytes: &[u8]) -> bool {
        // Some private keys start with a sequence. TODO: what are others
        bytes.starts_with(b"\x30")
    }

    /// Construct a private key from DER binary file.
    pub fn from_der(key_der: impl Into<Vec<u8>>) -> crate::Result<PrivateKey> {
        let key_der = key_der.into();
        // TODO: better assertion
        if key_der.is_empty() {
            return Err(crate::Error::new_other("empty private key"));
        }
        Ok(PrivateKey(key_der))
    }

    /// Construct a private key from PEM text file.
    ///
    /// This operation returns an error if PEM file contains zero or more than one certificate.
    pub fn from_pem(key_pem: impl AsRef<[u8]>) -> crate::Result<PrivateKey> {
        let pem = pem::parse_many(key_pem.as_ref());
        let count = pem.len();
        let mut keys: Vec<PrivateKey> = pem
            .into_iter()
            .flat_map(|p| match p.tag.as_ref() {
                "PRIVATE KEY" | "RSA PRIVATE KEY" => Some(Self::from_der(p.contents)),
                _ => None,
            })
            .collect::<Result<_, _>>()?;
        if keys.len() == 1 {
            return Ok(keys.swap_remove(0));
        } else if keys.len() > 1 {
            return Err(crate::Error::new_other("PEM file contains {} private keys"));
        } else if count != 0 {
            return Err(crate::Error::new_other(
                "PEM file contains {} entries, but no private keys",
            ));
        } else if Self::looks_like_der(key_pem.as_ref()) {
            return Err(crate::Error::new_other("PEM file looks like a DER file"));
        } else {
            return Err(crate::Error::new_other(
                "no private keys found in a PEM file",
            ));
        }
    }

    /// Get DER.
    pub fn get_der(&self) -> &[u8] {
        &self.0
    }

    /// Incorrect because it assumes it outputs `RSA PRIVATE KEY`
    /// without verifying that the private key is actually RSA.
    #[doc(hidden)]
    pub fn to_pem_incorrect(&self) -> String {
        pem::encode(&pem::Pem {
            tag: "RSA PRIVATE KEY".to_owned(),
            contents: self.0.clone(),
        })
    }
}

/// Parse PEM file into a pair of certificate and private key.
pub fn pem_to_cert_key_pair(pem: &[u8]) -> crate::Result<(Cert, PrivateKey)> {
    let entries = pem::parse_many(pem);
    if entries.len() != 2 {
        return Err(crate::Error::new_other(&format!(
            "PEM file should contain certificate and private key entries, got {} entries",
            entries.len()
        )));
    }
    let cert = Cert::from_pem(pem)?;
    let key = PrivateKey::from_pem(pem)?;
    Ok((cert, key))
}

/// DER-encoded
pub struct Pkcs12(pub Vec<u8>);

/// Pair of PKCS #12 and password.
pub struct Pkcs12AndPassword {
    /// PKCS #12 file, typically containing a certificate and private key.
    pub pkcs12: Pkcs12,
    /// Password for the file.
    pub password: String,
}
