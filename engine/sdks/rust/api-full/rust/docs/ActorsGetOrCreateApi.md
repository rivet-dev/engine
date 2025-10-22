# \ActorsGetOrCreateApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**actors_get_or_create**](ActorsGetOrCreateApi.md#actors_get_or_create) | **PUT** /actors | ## Datacenter Round Trips



## actors_get_or_create

> models::ActorsGetOrCreateResponse actors_get_or_create(namespace, actors_get_or_create_request)
## Datacenter Round Trips

**If actor exists**  2 round trips: - namespace::ops::resolve_for_name_global - GET /actors/{}  **If actor does not exist and is created in the current datacenter:**  2 round trips: - namespace::ops::resolve_for_name_global - [pegboard::workflows::actor] Create actor workflow (includes Epoxy key allocation)  **If actor does not exist and is created in a different datacenter:**  3 round trips: - namespace::ops::resolve_for_name_global - POST /actors to remote datacenter - [pegboard::workflows::actor] Create actor workflow (includes Epoxy key allocation)  actor::get will always be in the same datacenter.  ## Optimized Alternative Routes

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**namespace** | **String** |  | [required] |
**actors_get_or_create_request** | [**ActorsGetOrCreateRequest**](ActorsGetOrCreateRequest.md) |  | [required] |

### Return type

[**models::ActorsGetOrCreateResponse**](ActorsGetOrCreateResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

