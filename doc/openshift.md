# Openshift Configurations

## Install Kubeflow Notebook Operator

Clone [OpenDataHub](https://github.com/opendatahub-io/manifests) manifests
```shell
git clone https://github.com/opendatahub-io/manifests.git
```

Install the standalone Notebook CRD
```shell
cd manifests

kustomize build apps/jupyter/notebook-controller/upstream/overlays/standalone | kubectl apply -f -
```