# Patchify roadmap

## Version 0.1.0

### Server

  - [ ] **Configuration**
      - [ ] Name of application, to use with binary release files
      - [ ] Location of released binaries
      - [ ] List of versions, with corresponding hashes
      - [ ] Private key to use for signing
      - [ ] On start-up, sort the available versions in a manner that is
            sympathetic to version numbering, to determine the latest
  - [ ] **API endpoints**
      - [ ] Latest version number
          - [ ] `GET /latest`
          - [ ] Accept details of the version making the request
          - [ ] Provide the latest version number with signature
      - [ ] Releases
          - [ ] `GET /releases/:version`
          - [ ] Accept details of the version making the request
          - [ ] Provide the release binary for download
      - [ ] Verification information
          - [ ] `GET /hashes/:version`
          - [ ] Accept details of the version making the request
          - [ ] Confirm the authenticity of the binary by providing a SHA256
                hash with signature
  - [ ] **Security**
      - [ ] Private key will be used to provide a signature to the version
            number and SHA256 hash
  - [ ] **Errors and logging**
      - [ ] Log all errors
      - [ ] Log all requests
          - [ ] Log the versions making the requests
      - [ ] Check versions against provided binaries upon start-up
          - [ ] Exit with an error if there are any missing, or if any hashes
                don’t match
  - [ ] **Tests**
  - [ ] **Documentation**

### Client

  - [ ] **Configuration**
      - [ ] The version of the application
      - [ ] Where to look for updates — hostname plus basepath of API
      - [ ] How often to check for updates
          - [ ] Every time the application starts
          - [ ] Every X interval of time
      - [ ] Public key to use for verifying provided information
  - [ ] **Status**
      - [ ] Critical actions counter, with methods to increment and decrement
      - [ ] Central status enum — update at each stage
      - [ ] Method to check if an update has been started
      - [ ] Only start new critical actions if the status allows it
  - [ ] **Check for updates**
      - [ ] Query the server at intervals
          - [ ] Check no upgrade is currently in progress
          - [ ] Send details of the version in use
          - [ ] Check response against version in use
          - [ ] Use the public key to verify the legitimacy of the provided
                information
  - [ ] **Perform the update**
      - [ ] Make update status information available
          - [ ] Update central status enum at each stage
          - [ ] Broadcast the status updates to interested subscribers
      - [ ] Download the new binary
          - [ ] Save into a tmpdir — this will be auto-cleaned when the
                application exits
          - [ ] Provide information about the download progress
      - [ ] Verify the correctness of the downloaded binary
          - [ ] Run a SHA256 hash and compare with the published hash
          - [ ] Use the public key to verify the legitimacy of the provided
                information
      - [ ] Replace the installed application binary
          - [ ] This may need finesse under Windows
      - [ ] Shut down the current application activity
          - [ ] Update the central status to indicate that an upgrade is
                underway
          - [ ] Wait until the critical actions counter has reached zero, then
                trigger the restart
      - [ ] Start the new version
          - [ ] Check that the new version has started correctly
      - [ ] Exit the old version
  - [ ] **Errors and logging**
      - [ ] Log all attempts and their results
      - [ ] Log any issues such as failed verification of signed data
  - [ ] **Tests**
  - [ ] **Documentation**


## Version 0.2.0

### Server

  - [ ] Dynamic list of versions — with database support
  - [ ] Ranges of compatibility, e.g. getting the latest compatible version
        under semver rules
  - [ ] Version yanking
  - [ ] Differentiate which requests come from which active installations
  - [ ] Support for different types of binary, e.g. Windows, Linux, Mac
  - [ ] Support for different architectures, e.g. x86, x86_64, ARM
  - [ ] Support for different release channels, e.g. stable, beta, nightly
  - [ ] Support for different release formats, e.g. tarball, zip, deb, rpm

### Client

  - [ ] Support binaries and update info coming from separate places
  - [ ] Allow asking the user for permission to restart, for interactive
        applications
  - [ ] Support more nuanced or complex permissions setups for updating and
        restarting the application
  - [ ] Support downgrading
  - [ ] Support skipping versions
  - [ ] Support pinning versions
  - [ ] Support retrying of requests
  - [ ] Support rate-limiting of requests

