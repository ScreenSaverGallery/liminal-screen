# Liminal Screen API Security

## Overview

The Liminal Screen API includes optional security features to protect against unauthorized access to your screensaver application. This security system uses shared secret authentication to ensure only trusted remote options pages can communicate with your Liminal Screen installation.

## Security Model

### Threat Model

The API addresses several potential security risks:

1. **Impersonation Attacks**: Malicious sites pretending to be legitimate options pages
2. **Unauthorized Configuration Changes**: Unauthorized modification of screensaver settings
3. **Data Exfiltration**: Unauthorized access to application configuration
4. **Command Injection**: Exploitation of IPC communication channels

### Security Features

- **Shared Secret Authentication**: Mutual authentication between app and remote options
- **Token-Based Authorization**: Time-limited tokens prevent replay attacks
- **Environment Isolation**: Works only in authorized Tauri environments
- **Input Validation**: Server-side validation of all commands

## Enabling Security

### 1. Configure Shared Secret

Set the shared secret in your `.env` file:

```bash
# .env
LIMINAL_API_SECRET=your-very-long-random-secret-key-here
```

Generate a strong secret using a password generator or cryptographic tool:

```bash
# Generate a secure random secret
openssl rand -hex 32
```

### 2. Enable Authentication Requirement

Configure your Liminal Screen application to require authentication:

```javascript
// In your remote options page
import { liminalAPI } from '@liminal-screen/api';

// Configure security
liminalAPI.configureSecurity({
  sharedSecret: 'your-shared-secret-here',
  requireAuth: true
});

// Initialize the API
await liminalAPI.init();
```

### 3. Generate Authentication Tokens

When making API calls, include authentication tokens:

```javascript
// Generate auth token
const authToken = liminalAPI.generateAuthToken();

// Use token with API calls
await liminalAPI.setOptions({
  startsIn: 0.5,
  debug: true
}, authToken);
```

## How It Works

### Token Generation

1. **Timestamp Creation**: Current timestamp is captured
2. **Nonce Generation**: Cryptographically secure random nonce prevents replay attacks
3. **Signature Creation**: HMAC-SHA256 signature using shared secret via `crypto.subtle`
4. **Token Assembly**: Combined into verifiable token format

### Token Validation

1. **Timestamp Check**: Ensures token hasn't expired
2. **Signature Verification**: Validates token authenticity
3. **Replay Protection**: Checks against previously used tokens
4. **Session Management**: Maintains active session state

### Security Levels

#### Basic Security (Default)
- No authentication required
- Works in any Tauri environment
- Suitable for local/trusted network use

#### Enhanced Security
- Shared secret authentication required
- Time-limited tokens
- Replay attack protection
- Recommended for internet-facing deployments

## Best Practices

### Secret Management

1. **Never commit secrets to version control**
2. **Use different secrets for development and production**
3. **Rotate secrets periodically**
4. **Store secrets securely in environment variables**

### Token Handling

1. **Use short-lived tokens** (default 1 hour expiration)
2. **Generate new tokens for each sensitive operation**
3. **Never log or expose tokens in client-side code**
4. **Implement proper error handling for authentication failures**

### Network Security

1. **Serve options pages over HTTPS**
2. **Use Content Security Policy headers**
3. **Implement rate limiting on API endpoints**
4. **Monitor for suspicious authentication attempts**

## Example Implementation

```javascript
// secure-options.html
import { liminalAPI } from '@liminal-screen/api';

// Configure security — use a safe pattern that works in browsers too
liminalAPI.configureSecurity({
  sharedSecret:
    typeof process !== 'undefined' && process.env
      ? process.env.LIMINAL_API_SECRET
      : undefined,
  requireAuth: true
});

async function initializeSecureOptions() {
  try {
    // Initialize with security
    await liminalAPI.init();

    // Load options with authentication
    const authToken = liminalAPI.generateAuthToken();
    const options = await liminalAPI.getOptions(authToken);

    // Update UI with options
    updateOptionsForm(options);

  } catch (error) {
    if (error.name === 'LiminalAPIError') {
      console.error('Security error:', error.message);
      // Handle authentication failure
      showAuthenticationError();
    }
  }
}

async function saveSecureOptions(formData) {
  try {
    const authToken = liminalAPI.generateAuthToken();
    await liminalAPI.setOptions({
      startsIn: formData.startsIn,
      debug: formData.debug
    }, authToken);
    showSuccess('Settings saved securely!');
  } catch (error) {
    if (error.name === 'LiminalAPIError') {
      console.error('Failed to save settings:', error.message);
      showError('Authentication required to save settings');
    }
  }
}
```

## Troubleshooting

### Common Issues

#### "Authentication required but no shared secret configured"
- Ensure `LIMINAL_API_SECRET` is set in environment variables
- Verify the secret is accessible to the application
- Check that security is properly configured in the API client

#### "Invalid authentication signature"
- Verify shared secret matches between client and server
- Check for typos or encoding issues in the secret
- Ensure both sides are using the same HMAC-SHA256 algorithm

#### "Authentication token expired"
- Generate a new token (tokens expire after 1 hour by default)
- Check system clock synchronization
- Adjust session timeout if needed

### Security Monitoring

Enable logging to monitor authentication attempts:

```javascript
// Enable detailed security logging
localStorage.setItem('liminal-debug-security', 'true');
```

Logs will include:
- Successful authentications
- Failed authentication attempts
- Token generation events
- Replay attack detections

## Compliance Considerations

### Data Privacy
- Authentication tokens contain no personal information
- Shared secrets are never transmitted over networks
- All communication occurs over secure IPC channels

### Regulatory Compliance
- Follow organizational policies for secret management
- Implement audit logging for sensitive operations
- Regular security assessments of remote options pages

## Future Enhancements

Planned security improvements:

1. **Certificate-based authentication** for enterprise deployments
2. **Multi-factor authentication** for high-security environments
3. **OAuth integration** for third-party service authentication
4. **Advanced encryption** for token signing

Stay updated with the latest security patches by regularly updating the `@liminal-screen/api` package.