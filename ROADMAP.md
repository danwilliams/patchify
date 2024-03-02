# Patchify roadmap

## Version 0.1.0

### Server

  - [x] **Configuration**
      - [x] Name of application, to use with binary release files
      - [x] Location of released binaries
      - [x] List of versions, with corresponding hashes
      - [x] Private key to use for signing
      - [x] On start-up, sort the available versions in a manner that is
            sympathetic to version numbering, to determine the latest
  - [ ] **API endpoints**
      - [ ] Latest version number
          - [x] `GET /latest`
          - [ ] Accept details of the version making the request
          - [x] Provide the latest version number with signature
      - [ ] Releases
          - [x] `GET /releases/:version`
          - [ ] Accept details of the version making the request
          - [x] Provide the release binary for download
      - [ ] Verification information
          - [x] `GET /hashes/:version`
          - [ ] Accept details of the version making the request
          - [x] Confirm the authenticity of the binary by providing a SHA256
                hash with signature
  - [x] **Security**
      - [x] Private key will be used to provide a signature to the version
            number and SHA256 hash
  - [ ] **Errors and logging**
      - [x] Log all errors
      - [x] Log all requests
          - [ ] Log the versions making the requests
      - [x] Check versions against provided binaries upon start-up
          - [x] Exit with an error if there are any missing, or if any hashes
                don’t match
  - [ ] **Tests**
  - [x] **Documentation**

### Client

  - [x] **Configuration**
      - [x] The version of the application
      - [x] Where to look for updates — hostname plus basepath of API
      - [x] How often to check for updates
          - [x] Every time the application starts
          - [x] Every X interval of time
      - [x] Public key to use for verifying provided information
  - [x] **Status**
      - [x] Critical actions counter, with methods to increment and decrement
      - [x] Central status enum — update at each stage
      - [x] Method to check if an update has been started
      - [x] Only start new critical actions if the status allows it
  - [x] **Check for updates**
      - [x] Query the server at intervals
          - [x] Check no upgrade is currently in progress
          - [ ] Send details of the version in use
          - [x] Check response against version in use
          - [x] Use the public key to verify the legitimacy of the provided
                information
  - [ ] **Perform the update**
      - [x] Make update status information available
          - [x] Update central status enum at each stage
          - [x] Broadcast the status updates to interested subscribers
      - [x] Download the new binary
          - [x] Save into a tmpdir — this will be auto-cleaned when the
                application exits
          - [ ] Provide information about the download progress
      - [x] Verify the correctness of the downloaded binary
          - [x] Run a SHA256 hash and compare with the published hash
          - [x] Use the public key to verify the legitimacy of the provided
                information
      - [x] Replace the installed application binary
          - [ ] This may need finesse under Windows
      - [x] Shut down the current application activity
          - [x] Update the central status to indicate that an upgrade is
                underway
          - [x] Wait until the critical actions counter has reached zero, then
                trigger the restart
      - [x] Start the new version
          - [ ] Check that the new version has started correctly
      - [x] Exit the old version
  - [x] **Errors and logging**
      - [x] Log all attempts and their results
      - [x] Log any issues such as failed verification of signed data
  - [ ] **Tests**
  - [x] **Documentation**


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


