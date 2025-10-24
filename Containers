jobs:
    build:
        runs-on: ...
        steps:
            - uses: actions/checkout@v3

            # ... some other preparation steps (dependencies, compilation as a separate step, etc) ...

            - name: Setup Testcontainers Cloud Client
              uses: atomicjar/testcontainers-cloud-setup-action@v1
              with:
                  token: ${{ secrets.TC_CLOUD_TOKEN }}

            # ... existing steps executing your Testcontainers tests go here ...
