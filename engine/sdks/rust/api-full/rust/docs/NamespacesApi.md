# \NamespacesApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**namespaces_create**](NamespacesApi.md#namespaces_create) | **POST** /namespaces | 
[**namespaces_list**](NamespacesApi.md#namespaces_list) | **GET** /namespaces | 



## namespaces_create

> models::NamespacesCreateResponse namespaces_create(namespaces_create_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**namespaces_create_request** | [**NamespacesCreateRequest**](NamespacesCreateRequest.md) |  | [required] |

### Return type

[**models::NamespacesCreateResponse**](NamespacesCreateResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## namespaces_list

> models::NamespaceListResponse namespaces_list(limit, cursor, name, namespace_ids)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**limit** | Option<**i32**> |  |  |
**cursor** | Option<**String**> |  |  |
**name** | Option<**String**> |  |  |
**namespace_ids** | Option<**String**> |  |  |

### Return type

[**models::NamespaceListResponse**](NamespaceListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

