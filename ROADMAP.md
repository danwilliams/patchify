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
  - [x] **API endpoints**
      - [x] Latest version number
          - [x] `GET /latest`
          - [x] Provide the latest version number with signature
      - [x] Releases
          - [x] `GET /releases/:version`
          - [x] Provide the release binary for download
      - [x] Verification information
          - [x] `GET /hashes/:version`
          - [x] Confirm the authenticity of the binary by providing a SHA256
                hash with signature
  - [x] **Security**
      - [x] Private key will be used to provide a signature to the version
            number and SHA256 hash
  - [x] **Errors and logging**
      - [x] Log all errors
      - [x] Log all requests
      - [x] Check versions against provided binaries upon start-up
          - [x] Exit with an error if there are any missing, or if any hashes
                don’t match
  - [x] **Tests**
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
          - [x] Check response against version in use
          - [x] Use the public key to verify the legitimacy of the provided
                information
  - [x] **Perform the update**
      - [x] Make update status information available
          - [x] Update central status enum at each stage
          - [x] Broadcast the status updates to interested subscribers
      - [x] Download the new binary
          - [x] Save into a tmpdir — this will be auto-cleaned when the
                application exits
          - [x] Provide information about the download progress
      - [x] Verify the correctness of the downloaded binary
          - [x] Run a SHA256 hash and compare with the published hash
          - [x] Use the public key to verify the legitimacy of the provided
                information
      - [x] Replace the installed application binary
      - [x] Shut down the current application activity
          - [x] Update the central status to indicate that an upgrade is
                underway
          - [x] Wait until the critical actions counter has reached zero, then
                trigger the restart
      - [x] Start the new version
      - [x] Exit the old version
  - [x] **Errors and logging**
      - [x] Log all attempts and their results
      - [x] Log any issues such as failed verification of signed data
  - [x] **Tests**
  - [x] **Documentation**


## Version 0.2.0

### Server

  - [ ] **Tracking**
      - [ ] Accept the versions of the applications making the requests
      - [ ] Log the versions making the requests
      - [ ] Differentiate which requests come from which active installations
  - [ ] **Progress updates**
      - [ ] Progress updates for these areas:
          - [ ] Initial release checking
      - [ ] Configuration for how often to send an update (duration-based, or
            every time there’s an update)

### Client

  - [ ] **Auto-restart / manual control**
      - [ ] Make auto-restart optional
      - [ ] Make the restart method public
      - [ ] Make the update check public, for manual triggering
      - [ ] Allow asking the user for permission to restart, for interactive
            applications
  - [ ] **Tracking**
      - [ ] Send details of the version in use
      - [ ] Notify when updated successfully
      - [ ] Notify when update failed
  - [ ] **Progress updates**
      - [ ] Progress updates for these areas:
          - [x] Download
          - [ ] Verify
          - [ ] Copy (install)
          - [ ] Critical actions remaining
      - [ ] Configuration for how often to send an update (duration-based, or every
            time there’s an update)
  - [ ] **Error behaviour**
      - [ ] Retry or not
      - [ ] Retry N times before dropping to slower interval


## Future versions

### Server

  - [ ] **Enhanced HTTP support**
      - [ ] Support for range requests, allowing partial transfers
      - [ ] Support for resuming downloads
      - [ ] Support for segment hashing of files to allow for partial
            verification and resuming of downloads
      - [ ] Support HTTP compression
      - [ ] Support rate-limiting of requests
  - [ ] **Version management**
      - [ ] Dynamic list of versions — with database support
      - [ ] Ranges of compatibility, e.g. getting the latest compatible version
            under semver rules
      - [ ] Version yanking
      - [ ] Support for patches, i.e. partial file changes
      - [ ] Support for different types of binary, e.g. Windows, Linux, Mac
      - [ ] Support for different architectures, e.g. x86, x86_64, ARM
      - [ ] Support for different release channels, e.g. stable, beta, nightly
      - [ ] Support for different release formats, e.g. tarball, zip, deb, rpm

### Client

  - [ ] **Windows compatibility**
      - [ ] Replace the installed application binary
  - [ ] **Upgrade failure detection**
      - [ ] Check that the new version has started correctly
      - [ ] Roll back to the previous version if the new version fails to start
      - [ ] Check rollback status
      - [ ] Notify when rollback is successful
  - [ ] **Extended data**
      - [ ] Support binaries and update info coming from separate places
  - [ ] **Version management**
      - [ ] Support downgrading
      - [ ] Support skipping versions
      - [ ] Support pinning versions
  - [ ] **Complex installations**
      - [ ] Support more nuanced or complex permissions setups for updating and
            restarting the application


